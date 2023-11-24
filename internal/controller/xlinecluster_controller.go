/*
Copyright 2023.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controller

import (
	"context"
	"fmt"

	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	"github.com/xline-kv/xline-operator/internal/reconciler"
	"github.com/xline-kv/xline-operator/internal/util"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
)

// XlineClusterReconciler reconciles a XlineCluster object
type XlineClusterReconciler struct {
	client.Client
	Scheme *runtime.Scheme
}

//+kubebuilder:rbac:groups=xline.kvstore.datenlord.com,resources=xlineclusters,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=xline.kvstore.datenlord.com,resources=xlineclusters/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=xline.kvstore.datenlord.com,resources=xlineclusters/finalizers,verbs=update
//+kubebuilder:rbac:groups=core,resources=services,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=apps,resources=statefulsets,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=pods,verbs=get;list;watch

func (r *XlineClusterReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	recCtx := reconciler.NewReconcileContext(r.Client, r.Scheme, ctx)
	// obtain CR
	cr := &xapi.XlineCluster{}
	exist, err := recCtx.Exist(req.NamespacedName, cr)
	if err != nil {
		return ctrl.Result{Requeue: true}, err
	}
	// skip reconciling process when it has been deleted
	if !exist {
		recCtx.Log.Info(fmt.Sprintf("XlineCluster(%s) has been deleted", util.K8sObjKeyStr(req.NamespacedName)))
		return ctrl.Result{}, nil
	}
	rec := reconciler.XlineClusterReconciler{ReconcileContext: recCtx, CR: cr}

	curSpecHash := util.Md5HashOr(cr.Spec, "")
	isFirstCreated := cr.Status.LastApplySpecHash == nil
	specHasChanged := isFirstCreated || *cr.Status.LastApplySpecHash != curSpecHash
	preRecCompleted := cr.Status.Stage == xapi.StageComplete

	if isFirstCreated && cr.Status.Stage == "" {
		recCtx.Log.Info(fmt.Sprintf("XlineCluster(%s) is created for the first time", util.K8sObjKeyStr(req.NamespacedName)))
	}
	if specHasChanged {
		recCtx.Log.Info(fmt.Sprintf("XlineCluster(%s) spec has been updated", util.K8sObjKeyStr(req.NamespacedName)))
	}

	// reconcile the sub resource of XlineCluster
	var recErr error
	if specHasChanged || !preRecCompleted {
		recRs := rec.Reconcile()
		recErr = recRs.Err
		cr.Status.XlineClusterRecStatus = recRs.AsXlineClusterRecStatus()
		// when reconcile process competed success, update the last apply spec hash
		if recRs.Stage == xapi.StageComplete {
			cr.Status.LastApplySpecHash = &curSpecHash
		}
	}
	// sync the status of CR
	syncRs, syncErr := rec.Sync()
	cr.Status.XlineClusterSyncStatus = syncRs
	// update status
	updateErr := r.Status().Update(ctx, cr)

	// merge error at different reconcile phases
	errSet := StCtrlErrSet{
		Rec:    recErr,
		Sync:   syncErr,
		Update: updateErr,
	}
	return errSet.AsResult()
}

// SetupWithManager sets up the controller with the Manager.
func (r *XlineClusterReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&xapi.XlineCluster{}).
		Complete(r)
}
