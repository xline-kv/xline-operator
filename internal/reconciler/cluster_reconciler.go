/*
 *
 * Copyright 2023
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * /
 */

package reconciler

import (
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	"github.com/xline-kv/xline-operator/internal/constants"
	tran "github.com/xline-kv/xline-operator/internal/transformer"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	rbacv1 "k8s.io/api/rbac/v1"
	"k8s.io/apimachinery/pkg/types"
)

// XlineClusterReconciler reconciles a XlineCluster object
type XlineClusterReconciler struct {
	ReconcileContext
	CR *xapi.XlineCluster
}

// ClusterStageRecResult represents the result of a stage reconciliation for XlineCluster
type ClusterStageRecResult struct {
	Stage  xapi.XlineClusterOprStage
	Status xapi.OprStageStatus
	Err    error
}

func clusterStageSucc(stage xapi.XlineClusterOprStage) ClusterStageRecResult {
	return ClusterStageRecResult{Stage: stage, Status: xapi.StageResultSucceeded}
}

func clusterStageFail(stage xapi.XlineClusterOprStage, err error) ClusterStageRecResult {
	return ClusterStageRecResult{Stage: stage, Status: xapi.StageResultFailed, Err: err}
}

// Reconcile all sub components
func (r *XlineClusterReconciler) Reconcile() ClusterStageRecResult {
	result := r.recXlineResources()
	if result.Err != nil {
		return result
	}
	return ClusterStageRecResult{Stage: xapi.StageComplete, Status: xapi.StageResultSucceeded}
}

func (r *ClusterStageRecResult) AsXlineClusterRecStatus() xapi.XlineClusterRecStatus {
	res := xapi.XlineClusterRecStatus{
		Stage:       r.Stage,
		StageStatus: r.Status,
	}
	if r.Err != nil {
		res.LastMessage = r.Err.Error()
	}
	return res
}

// reconcile xline cluster resources.
func (r *XlineClusterReconciler) recXlineResources() ClusterStageRecResult {
	// create an xline discovery service
	discoverySvc := tran.MakeDiscoveryService(r.CR, r.Schema)
	if err := r.CreateOrUpdate(discoverySvc, &corev1.Service{}); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoveryService, err)
	}

	// create an xline discovery serviceaccount
	discoverySa := tran.MakeDiscoverySA(r.CR, r.Schema)
	if err := r.CreateOrUpdate(discoverySa, &corev1.ServiceAccount{}); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoverySA, err)
	}

	// create an xline discovery role
	discoveryRole := tran.MakeDiscoveryRole(r.CR, r.Schema)
	if err := r.CreateOrUpdate(discoveryRole, &rbacv1.Role{}); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoveryRole, err)
	}

	// create a rolebinding for xline discovery
	discoveryRB := tran.MakeDiscoveryRoleBinding(r.CR, r.Schema)
	if err := r.CreateOrUpdate(discoveryRB, &rbacv1.RoleBinding{}); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoveryRoleBinding, err)
	}

	// create an xline discovery deployment
	mgrDeployName := types.NamespacedName{Name: constants.OperatorDeployName, Namespace: constants.OperatorNamespace}
	mgrDeploy := &appv1.Deployment{}
	if err := r.Get(r.Ctx, mgrDeployName, mgrDeploy); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoveryDeploy, err)
	}
	discoveryImage := mgrDeploy.Spec.Template.Spec.Containers[1].Image
	discoveryDeploy := tran.MakeDiscoveryDeployment(r.CR, r.Schema, discoveryImage)
	if err := r.CreateOrUpdate(discoveryDeploy, &appv1.Deployment{}); err != nil {
		return clusterStageFail(xapi.StageXlineDiscoveryDeploy, err)
	}

	// create an xline script cm
	script := tran.MakeScriptCM(r.CR, r.Schema)
	if err := r.CreateOrUpdate(script, &corev1.ConfigMap{}); err != nil {
		return clusterStageFail(xapi.StageXlineScriptCM, err)
	}
	// create an xline configmap
	configMap := tran.MakeConfigMap(r.CR, r.Schema)
	if err := r.CreateOrUpdate(configMap, &corev1.ConfigMap{}); err != nil {
		return clusterStageFail(xapi.StageXlineConfigMap, err)
	}
	// create an xline service
	service := tran.MakeService(r.CR, r.Schema)
	if err := r.CreateOrUpdate(service, &corev1.Service{}); err != nil {
		return clusterStageFail(xapi.StageXlineService, err)
	}
	// create an xline statefulset
	statefulSet := tran.MakeStatefulSet(r.CR, r.Schema)
	if err := r.CreateOrUpdate(statefulSet, &appv1.StatefulSet{}); err != nil {
		return clusterStageFail(xapi.StageXlineStatefulSet, err)
	}
	return clusterStageSucc(xapi.StageComplete)
}
