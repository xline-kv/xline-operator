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
	"encoding/json"
	"fmt"

	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
)

// NOTE: json tags are required.  Any new fields you add must have json tags for the fields to be serialized.

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

type XlineArgs struct {
	JaegerOffline    bool    `json:"JaegerOffline,omitempty"`
	JaegerOnline     bool    `json:"JaegerOnline,omitempty"`
	ClientUseBackoff bool    `json:"ClientUseBackoff,omitempty"`
	JaegerLevel      *string `json:"JaegerLevel,omitempty"`
	JaegerOutputDir  *string `json:"JaegerOutputDir,omitempty"`
	LogFile          *string `json:"LogFile,omitempty"`

	// +kubebuilder:validation:Enum=never;hourly;daily
	LogRotate *string `json:"LogRotate,omitempty"`

	// +kubebuilder:validation:Enum=trace;debug;info;warn;error
	LogLevel *string `json:"LogLevel,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms)
	HeartbeatInterval *string `json:"HeartbeatInterval,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(ms|s)
	ServerWaitSyncedTimeout *string `json:"ServerWaitSyncedTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(ms|s)
	RetryTimeout *string `json:"RetryTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	RpcTimeout *string `json:"RpcTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	BatchTimeout *string `json:"BatchTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	ClientWaitSyncedTimeout *string `json:"ClientWaitSyncedTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	ClientProposeTimeout *string `json:"ClientProposeTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	ClientInitialRetryTimeout *string `json:"ClientInitialRetryTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	ClientMaxRetryTimeout *string `json:"ClientMaxRetryTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	GcInterval *string `json:"GcInterval,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	RangeRetryTimeout *string `json:"RangeRetryTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	CompactTimeout *string `json:"CompactTimeout,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	SyncVictimsInterval *string `json:"SyncVictimsInterval,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	WatchProgressNotifyInterval *string `json:"WatchProgressNotifyInterval,omitempty"`
	CurpDir                     *string `json:"CurpDir,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(us|ms|s)
	CompactSleepInterval *string `json:"CompactSleepInterval,omitempty"`
	// +kubebuilder:validation:Pattern=\d+(KB|MB|kb|mb)
	BatchMaxSize *string `json:"BatchMaxSize,omitempty"`
	// +kubebuilder:validation:Minimum=1
	RetryCount int `json:"RetryCount,omitempty"`
	// +kubebuilder:validation:Minimum=1
	FollowerTimeoutTicks int `json:"FollowerTimeoutTicks,omitempty"`
	// +kubebuilder:validation:Minimum=1
	CandidateTimeoutTicks int `json:"CandidateTimeoutTicks,omitempty"`
	// +kubebuilder:validation:Minimum=1
	LogEntriesCap int `json:"LogEntriesCap,omitempty"`
	// +kubebuilder:validation:Minimum=1
	CmdWorkers int `json:"CmdWorkers,omitempty"`
	// +kubebuilder:validation:Minimum=1
	CompactBatchSize int `json:"CompactBatchSize,omitempty"`
	// +kubebuilder:validation:Minimum=1
	Quota int `json:"Quota,omitempty"`
}

// ########################################
//   		XlineClusterSpec
// ########################################

// XlineClusterSpec defines the desired state of XlineCluster
// +k8s:openapi-gen=true
type XlineClusterSpec struct {
	// Xline cluster image
	Image *string `json:"image,omitempty"`

	/// Xline container bootstrap arguments
	/// Set additional arguments except [`--name`, `--members`, `--storage-engine`, `--data-dir`]
	BootstrapArgs *XlineArgs `json:"config,omitempty"`

	// ImagePullPolicy of Xline cluster Pods
	// +optional
	ImagePullPolicy corev1.PullPolicy `json:"imagePullPolicy,omitempty"`

	// The replicas of xline nodes
	// +kubebuilder:validation:Minimum=3
	Replicas int32 `json:"replicas"`

	// The auth secret keys
	AuthSecrets *XlineAuthSecret `json:"authSecret,omitempty"`

	// K8s storage-class-name of the Xline storage
	// Defaults to Kubernetes default storage class.
	// +optional
	StorageClassName *string `json:"storageClassName"`

	// Defines the specification of resource cpu, mem, storage.
	corev1.ResourceRequirements `json:",inline"`
}

type XlineAuthSecret struct {
	Name      *string `json:"name"`
	MountPath *string `json:"mountPath"`
	PubKey    *string `json:"pubKey"`
	PriKey    *string `json:"priKey"`
}

func (s *XlineClusterSpec) BootArgs() map[string]string {
	bytes, err := json.Marshal(s.BootstrapArgs)
	args := map[string]string{}
	if err != nil {
		return args
	}
	var data map[string]interface{}
	if json.Unmarshal(bytes, &data) != nil {
		return args
	}
	for k, v := range data {
		args[k] = fmt.Sprintf("%v", v)
	}
	return args
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
	StageXlineScriptCM    XlineClusterOprStage = "Xline/ScriptCM"
	StageXlineConfigMap   XlineClusterOprStage = "Xline/ConfigMap"
	StageXlineService     XlineClusterOprStage = "Xline/Service"
	StageXlineStatefulSet XlineClusterOprStage = "Xline/Statefulset"
	StageComplete         XlineClusterOprStage = "complete"
)

// XlineClusterRecStatus represents XlineCluster reconcile status
type XlineClusterRecStatus struct {
	Stage       XlineClusterOprStage `json:"stage,omitempty"`
	StageStatus OprStageStatus       `json:"stageStatus,omitempty"`
	LastMessage string               `json:"lastMessage,omitempty"`
}

// XlineClusterSyncStatus represents XlineCluster sync status
type XlineClusterSyncStatus struct {
	Image          string                       `json:"image,omitempty"`
	StatefulSetRef NamespacedName               `json:"statefulSetRef,omitempty"`
	ServiceRef     NamespacedName               `json:"serviceRef,omitempty"`
	Conditions     []appv1.StatefulSetCondition `json:"conditions,omitempty"`
}

func init() {
	SchemeBuilder.Register(&XlineCluster{}, &XlineClusterList{})
}
