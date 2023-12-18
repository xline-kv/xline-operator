package reconciler

import (
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	tran "github.com/xline-kv/xline-operator/internal/transformer"
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
	svcRef := tran.GetServiceKey(r.CR.ObjKey())
	svc := &corev1.Service{}
	exist, err := r.Exist(svcRef, svc)
	if err != nil {
		return err
	}
	if exist {
		xlineStatus.ServiceRef = xapi.NewNamespacedName(svcRef)
	}

	stsRef := tran.GetStatefulSetKey(r.CR.ObjKey())
	sts := &appv1.StatefulSet{}
	exist, err = r.Exist(stsRef, sts)
	if err != nil {
		return err
	}
	if exist {
		xlineStatus.Image = tran.GetXlineImage(r.CR)
		xlineStatus.StatefulSetRef = xapi.NewNamespacedName(stsRef)
		xlineStatus.Conditions = sts.Status.Conditions
	}

	return nil
}
