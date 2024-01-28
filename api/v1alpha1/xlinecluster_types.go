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
	"strconv"

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

// XlineArgs
// +k8s:openapi-gen=true
type XlineArgs struct {
	IsLeader                    bool    `json:"isLeader,omitempty"`
	JaegerOffline               bool    `json:"jaegerOffline,omitempty"`
	JaegerOnline                bool    `json:"jaegerOnline,omitempty"`
	JaegerLevel                 bool    `json:"jaegerLevel,omitempty"`
	ClientUseBackoff            bool    `json:"clientUseBackoff,omitempty"`
	AuthPrivateKey              *string `json:"authPrivateKey,omitempty"`
	AuthPublicKey               *string `json:"authPublicKey,omitempty"`
	JaegerOutputDir             *string `json:"jaegerOutputDir,omitempty"`
	LogFile                     *string `json:"logFile,omitempty"`
	LogRotate                   *string `json:"logRotate,omitempty"`
	LogLevel                    *string `json:"logLevel,omitempty"`
	HeartbeatInterval           *string `json:"heartbeatInterval,omitempty"`
	ServerWaitSyncedTimeout     *string `json:"serverWaitSyncedTimeout,omitempty"`
	RetryTimeout                *string `json:"retryTimeout,omitempty"`
	RpcTimeout                  *string `json:"rpcTimeout,omitempty"`
	BatchTimeout                *string `json:"batchTimeout,omitempty"`
	ClientWaitSyncedTimeout     *string `json:"clientWaitSyncedTimeout,omitempty"`
	ClientProposeTimeout        *string `json:"clientProposeTimeout,omitempty"`
	ClientInitialRetryTimeout   *string `json:"clientInitialRetryTimeout,omitempty"`
	ClientMaxRetryTimeout       *string `json:"clientMaxRetryTimeout,omitempty"`
	GcInterval                  *string `json:"gcInterval,omitempty"`
	RangeRetryTimeout           *string `json:"rangeRetryTimeout,omitempty"`
	CompactTimeout              *string `json:"compactTimeout,omitempty"`
	SyncVictimsInterval         *string `json:"syncVictimsInterval,omitempty"`
	WatchProgressNotifyInterval *string `json:"watchProgressNotifyInterval,omitempty"`
	CurpDir                     *string `json:"curpDir,omitempty"`
	CompactSleepInterval        *string `json:"compactSleepInterval,omitempty"`
	AutoCompactMode             *string `json:"autoCompactMode,omitempty"`
	AutoPeriodicRetention       *string `json:"autoPeriodicRetention,omitempty"`
	AutoRevisionRetention       *string `json:"autoRevisionRetention,omitempty"`
	InitialClusterState         *string `json:"initialClusterState,omitempty"`
	RetryCount                  *int    `json:"retryCount,omitempty"`
	BatchMaxSize                *int    `json:"batchMaxSize,omitempty"`
	FollowerTimeoutTicks        *int    `json:"followerTimeoutTicks,omitempty"`
	CandidateTimeoutTicks       *int    `json:"candidateTimeoutTicks,omitempty"`
	LogEntriesCap               *int    `json:"logEntriesCap,omitempty"`
	CmdWorkers                  *int    `json:"cmdWorkers,omitempty"`
	CompactBatchSize            *int    `json:"compactBatchSize,omitempty"`
	Quota                       *int    `json:"quota,omitempty"`
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

//nolint:gocyclo // seems not bad
func (s *XlineClusterSpec) BootArgs() []string {
	args := make([]string, 0)
	if s.BootstrapArgs.IsLeader {
		args = append(args, "--is-leader")
	}
	if s.BootstrapArgs.JaegerOffline {
		args = append(args, "--jaeger-offline")
	}
	if s.BootstrapArgs.JaegerOnline {
		args = append(args, "--jaeger-online")
	}
	if s.BootstrapArgs.JaegerLevel {
		args = append(args, "--jaeger-level")
	}
	if s.BootstrapArgs.ClientUseBackoff {
		args = append(args, "--client-use-backoff")
	}
	if s.BootstrapArgs.AuthPrivateKey != nil {
		args = append(args, "--auth-private-key", *s.BootstrapArgs.AuthPrivateKey)
	}
	if s.BootstrapArgs.AuthPublicKey != nil {
		args = append(args, "--auth-public-key", *s.BootstrapArgs.AuthPublicKey)
	}
	if s.BootstrapArgs.JaegerOutputDir != nil {
		args = append(args, "--jaeger-output-dir", *s.BootstrapArgs.JaegerOutputDir)
	}
	if s.BootstrapArgs.LogFile != nil {
		args = append(args, "--log-file", *s.BootstrapArgs.LogFile)
	}
	if s.BootstrapArgs.LogRotate != nil {
		args = append(args, "--log-rotate", *s.BootstrapArgs.LogRotate)
	}
	if s.BootstrapArgs.LogLevel != nil {
		args = append(args, "--log-level", *s.BootstrapArgs.LogLevel)
	}
	if s.BootstrapArgs.HeartbeatInterval != nil {
		args = append(args, "--heartbeat-interval", *s.BootstrapArgs.HeartbeatInterval)
	}
	if s.BootstrapArgs.ServerWaitSyncedTimeout != nil {
		args = append(args, "--server-wait-synced-timeout", *s.BootstrapArgs.ServerWaitSyncedTimeout)
	}
	if s.BootstrapArgs.RetryTimeout != nil {
		args = append(args, "--retry-timeout", *s.BootstrapArgs.RetryTimeout)
	}
	if s.BootstrapArgs.RpcTimeout != nil {
		args = append(args, "--rpc-timeout", *s.BootstrapArgs.RpcTimeout)
	}
	if s.BootstrapArgs.BatchTimeout != nil {
		args = append(args, "--batch-timeout", *s.BootstrapArgs.BatchTimeout)
	}
	if s.BootstrapArgs.ClientWaitSyncedTimeout != nil {
		args = append(args, "--client-wait-synced-timeout", *s.BootstrapArgs.ClientWaitSyncedTimeout)
	}
	if s.BootstrapArgs.ClientProposeTimeout != nil {
		args = append(args, "--client-propose-timeout", *s.BootstrapArgs.ClientProposeTimeout)
	}
	if s.BootstrapArgs.ClientInitialRetryTimeout != nil {
		args = append(args, "--client-initial-retry-timeout", *s.BootstrapArgs.ClientInitialRetryTimeout)
	}
	if s.BootstrapArgs.ClientMaxRetryTimeout != nil {
		args = append(args, "--client-max-retry-timeout", *s.BootstrapArgs.ClientMaxRetryTimeout)
	}
	if s.BootstrapArgs.GcInterval != nil {
		args = append(args, "--gc-interval", *s.BootstrapArgs.GcInterval)
	}
	if s.BootstrapArgs.RangeRetryTimeout != nil {
		args = append(args, "--range-retry-timeout", *s.BootstrapArgs.RangeRetryTimeout)
	}
	if s.BootstrapArgs.CompactTimeout != nil {
		args = append(args, "--compact-timeout", *s.BootstrapArgs.CompactTimeout)
	}
	if s.BootstrapArgs.SyncVictimsInterval != nil {
		args = append(args, "--sync-victims-interval", *s.BootstrapArgs.SyncVictimsInterval)
	}
	if s.BootstrapArgs.WatchProgressNotifyInterval != nil {
		args = append(args, "--watch-progress-notify-interval", *s.BootstrapArgs.WatchProgressNotifyInterval)
	}
	if s.BootstrapArgs.CurpDir != nil {
		args = append(args, "--curp-dir", *s.BootstrapArgs.CurpDir)
	}
	if s.BootstrapArgs.CompactSleepInterval != nil {
		args = append(args, "--compact-sleep-interval", *s.BootstrapArgs.CompactSleepInterval)
	}
	if s.BootstrapArgs.AutoCompactMode != nil {
		args = append(args, "--auto-compact-mode", *s.BootstrapArgs.AutoCompactMode)
	}
	if s.BootstrapArgs.AutoPeriodicRetention != nil {
		args = append(args, "--auto-periodic-retention", *s.BootstrapArgs.AutoPeriodicRetention)
	}
	if s.BootstrapArgs.AutoRevisionRetention != nil {
		args = append(args, "--auto-revision-retention", *s.BootstrapArgs.AutoRevisionRetention)
	}
	if s.BootstrapArgs.InitialClusterState != nil {
		args = append(args, "--initial-cluster-state", *s.BootstrapArgs.InitialClusterState)
	}
	if s.BootstrapArgs.RetryCount != nil {
		args = append(args, "--retry-count", strconv.Itoa(*s.BootstrapArgs.RetryCount))
	}
	if s.BootstrapArgs.BatchMaxSize != nil {
		args = append(args, "--batch-max-size", strconv.Itoa(*s.BootstrapArgs.BatchMaxSize))
	}
	if s.BootstrapArgs.FollowerTimeoutTicks != nil {
		args = append(args, "--follower-timeout-ticks", strconv.Itoa(*s.BootstrapArgs.FollowerTimeoutTicks))
	}
	if s.BootstrapArgs.CandidateTimeoutTicks != nil {
		args = append(args, "--candidate-timeout-ticks", strconv.Itoa(*s.BootstrapArgs.CandidateTimeoutTicks))
	}
	if s.BootstrapArgs.LogEntriesCap != nil {
		args = append(args, "--log-entries-cap", strconv.Itoa(*s.BootstrapArgs.LogEntriesCap))
	}
	if s.BootstrapArgs.CmdWorkers != nil {
		args = append(args, "--cmd-workers", strconv.Itoa(*s.BootstrapArgs.CmdWorkers))
	}
	if s.BootstrapArgs.CompactBatchSize != nil {
		args = append(args, "--compact-batch-size", strconv.Itoa(*s.BootstrapArgs.CompactBatchSize))
	}
	if s.BootstrapArgs.Quota != nil {
		args = append(args, "--quota", strconv.Itoa(*s.BootstrapArgs.Quota))
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
