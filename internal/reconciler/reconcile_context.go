package reconciler

import (
	"context"

	"github.com/go-logr/logr"
	"github.com/xline-kv/xline-operator/internal/util"
	"k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
)

// ReconcileContext is the context for reconciling CRD.
type ReconcileContext struct {
	client.Client
	Schema *runtime.Scheme
	Ctx    context.Context
	Log    logr.Logger
}

func NewReconcileContext(client client.Client, schema *runtime.Scheme, ctx context.Context) ReconcileContext {
	return ReconcileContext{
		Client: client,
		Schema: schema,
		Ctx:    ctx,
		Log:    log.FromContext(ctx),
	}
}

// CreateOrUpdate creates or updates the kubernetes object.
func (r *ReconcileContext) CreateOrUpdate(obj client.Object, objType client.Object) error {
	key := client.ObjectKeyFromObject(obj)
	exist, err := r.Exist(key, objType)
	if err != nil {
		return err
	}
	if !exist {
		// create object
		if err := r.Create(r.Ctx, obj); err != nil {
			return err
		}
		r.Log.Info("create object: " + util.K8sObjKeyStr(key))
		return nil
	} else {
		return r.Update(r.Ctx, obj)
	}
}

// Exist checks if the kubernetes object exists.
func (r *ReconcileContext) Exist(key types.NamespacedName, objType client.Object) (bool, error) {
	if err := r.Get(r.Ctx, key, objType); err != nil {
		if errors.IsNotFound(err) {
			return false, nil
		}
		return false, err
	}
	return true, nil
}

// DeleteWhenExist deletes the kubernetes object if it exists.
func (r *ReconcileContext) DeleteWhenExist(key types.NamespacedName, objType client.Object, deleteOpts ...client.DeleteOption) error {
	exist, err := r.Exist(key, objType)
	if err != nil {
		return err
	}
	if exist {
		if err := r.Delete(r.Ctx, objType, deleteOpts...); err != nil {
			return err
		}
		r.Log.Info("delete object: " + util.K8sObjKeyStr(key))
	}
	return nil
}
