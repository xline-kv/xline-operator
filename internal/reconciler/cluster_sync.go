package reconciler

import (
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
)

// Sync all subcomponents status.
func (r *XlineClusterReconciler) Sync() (xapi.XlineClusterSyncStatus, error) {
	syncRes := &xapi.XlineClusterSyncStatus{}
	err := r.syncXlineStatus(syncRes)
	return *syncRes, err
}

// sync XlineCluster status
func (r *XlineClusterReconciler) syncXlineStatus(xlineStatus *xapi.XlineClusterSyncStatus) error {
	xcLookupKey := r.CR.ObjKey()
	svc := &corev1.Service{}
	exist, err := r.Exist(xcLookupKey, svc)
	if err != nil {
		return err
	}
	if exist {
		xlineStatus.ServiceRef = xapi.NewNamespacedName(xcLookupKey)
	}

	sts := &appv1.StatefulSet{}
	exist, err = r.Exist(xcLookupKey, sts)
	if err != nil {
		return err
	}
	if exist {
		xlineStatus.Image = *r.CR.Spec.Image
		xlineStatus.StatefulSetRef = xapi.NewNamespacedName(xcLookupKey)
		xlineStatus.Conditions = sts.Status.Conditions
	}

	return nil
}
