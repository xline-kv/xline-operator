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
	tran "github.com/xline-kv/xline-operator/internal/transformer"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
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
	// create a xline service
	service := tran.MakeService(r.CR, r.Schema)
	if err := r.CreateOrUpdate(service, &corev1.Service{}); err != nil {
		return clusterStageFail(xapi.StageXlineService, err)
	}
	// create a xline statefulset
	statefulSet := tran.MakeStatefulSet(r.CR, r.Schema)
	if err := r.CreateOrUpdate(statefulSet, &appv1.StatefulSet{}); err != nil {
		return clusterStageFail(xapi.StageXlineStatefulSet, err)
	}
	return clusterStageSucc(xapi.StageComplete)

}
