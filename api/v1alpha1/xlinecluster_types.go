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
	JaegerOffline               bool    `json:"jaeger-offline,omitempty"`
	JaegerOnline                bool    `json:"jaeger-online,omitempty"`
	JaegerLevel                 bool    `json:"jaeger-level,omitempty"`
	ClientUseBackoff            bool    `json:"client-use-backoff,omitempty"`
	AuthPrivateKey              *string `json:"auth-private-key,omitempty"`
	AuthPublicKey               *string `json:"auth-public-key,omitempty"`
	JaegerOutputDir             *string `json:"jaeger-output-dir,omitempty"`
	LogFile                     *string `json:"log-file,omitempty"`
	LogRotate                   *string `json:"log-rotate,omitempty"`
	LogLevel                    *string `json:"log-level,omitempty"`
	HeartbeatInterval           *string `json:"heartbeat-interval,omitempty"`
	ServerWaitSyncedTimeout     *string `json:"server-wait-synced-timeout,omitempty"`
	RetryTimeout                *string `json:"retry-timeout,omitempty"`
	RpcTimeout                  *string `json:"rpc-timeout,omitempty"`
	BatchTimeout                *string `json:"batch-timeout,omitempty"`
	ClientWaitSyncedTimeout     *string `json:"client-wait-synced-timeout,omitempty"`
	ClientProposeTimeout        *string `json:"client-propose-timeout,omitempty"`
	ClientInitialRetryTimeout   *string `json:"client-initial-retry-timeout,omitempty"`
	ClientMaxRetryTimeout       *string `json:"client-max-retry-timeout,omitempty"`
	GcInterval                  *string `json:"gc-interval,omitempty"`
	RangeRetryTimeout           *string `json:"range-retry-timeout,omitempty"`
	CompactTimeout              *string `json:"compact-timeout,omitempty"`
	SyncVictimsInterval         *string `json:"sync-victims-interval,omitempty"`
	WatchProgressNotifyInterval *string `json:"watch-progress-notify-interval,omitempty"`
	CurpDir                     *string `json:"curp-dir,omitempty"`
	CompactSleepInterval        *string `json:"compact-sleep-interval,omitempty"`
	RetryCount                  int     `json:"retry-count,omitempty"`
	BatchMaxSize                int     `json:"batch-max-size,omitempty"`
	FollowerTimeoutTicks        int     `json:"follower-timeout-ticks,omitempty"`
	CandidateTimeoutTicks       int     `json:"candidate-timeout-ticks,omitempty"`
	LogEntriesCap               int     `json:"log-entries-cap,omitempty"`
	CmdWorkers                  int     `json:"cmd-workers,omitempty"`
	CompactBatchSize            int     `json:"compact-batch-size,omitempty"`
	Quota                       int     `json:"quota,omitempty"`
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
	BootstrapArgs XlineArgs `json:"bootstrapArgs,omitempty"`

	// ImagePullPolicy of Xline cluster Pods
	// +optional
	ImagePullPolicy corev1.PullPolicy `json:"imagePullPolicy,omitempty"`

	// The replicas of xline nodes
	// +kubebuilder:validation:Minimum=3
	Replicas int32 `json:"replicas"`
}

func (s *XlineClusterSpec) BootArgs() []string {
	bytes, err := json.Marshal(s.BootstrapArgs)
	args := make([]string, 0)
	if err != nil {
		return args
	}
	var data map[string]interface{}
	if json.Unmarshal(bytes, &data) != nil {
		return args
	}
	for k, v := range data {
		if bv, ok := v.(bool); ok && bv {
			args = append(args, fmt.Sprintf("--%s", k))
			continue
		}
		args = append(args, fmt.Sprintf("--%s", k), fmt.Sprintf("%v", v))
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
