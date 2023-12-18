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

package v1alpha1

import (
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
)

// XlineCluster is the Schema for the xlineclusters API
// +kubebuilder:object:root=true
// +kubebuilder:subresource:status
// +kubebuilder:resource:shortName=xc

type XlineCluster struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   XlineClusterSpec      `json:"spec,omitempty"`
	Status XlineClusterStatus    `json:"status,omitempty"`
	objKey *types.NamespacedName `json:"-"`
}

// XlineClusterList contains a list of XlineCluster
// +kubebuilder:object:root=true
type XlineClusterList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []XlineCluster `json:"items"`
}

// ########################################
//   		XlineClusterSpec
// ########################################

// XlineClusterSpec defines the desired state of XlineCluster
// +k8s:openapi-gen=true
type XlineClusterSpec struct {
	// Xline cluster image version
	Version string `json:"version"`

	// Xline cluster image
	Image *string `json:"image,omitempty"`

	// ImagePullPolicy of Xline cluster Pods
	// +optional
	ImagePullPolicy corev1.PullPolicy `json:"imagePullPolicy,omitempty"`

	// The replicas of xline nodes
	// +kubebuilder:validation:Minimum=3
	Replicas int32 `json:"replicas"`
}

// XlineClusterStatus defines the observed state of XlineCluster
type XlineClusterStatus struct {
	LastApplySpecHash      *string `json:"lastApplySpecHash,omitempty"`
	XlineClusterRecStatus  `json:",inline"`
	XlineClusterSyncStatus `json:",inline"`
}

// XlineClusterOprStage represents XlineCluster operator stage
type XlineClusterOprStage string

const (
	StageXlineService     XlineClusterOprStage = "Xline/Service"
	StageXlineStatefulSet XlineClusterOprStage = "Xline/Statefulset"
	StageComplete         XlineClusterOprStage = "complete"
)

type XlineClusterRecStatus struct {
	Stage       XlineClusterOprStage `json:"stage,omitempty"`
	StageStatus OprStageStatus       `json:"stageStatus,omitempty"`
	LastMessage string               `json:"lastMessage,omitempty"`
}

type XlineClusterSyncStatus struct {
	Image          string                       `json:"image,omitempty"`
	StatefulSetRef NamespacedName               `json:"statefulSetRef,omitempty"`
	ServiceRef     NamespacedName               `json:"serviceRef,omitempty"`
	Conditions     []appv1.StatefulSetCondition `json:"conditions,omitempty"`
}

func init() {
	SchemeBuilder.Register(&XlineCluster{}, &XlineClusterList{})
}
