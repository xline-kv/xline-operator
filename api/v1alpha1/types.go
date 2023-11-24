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

import "k8s.io/apimachinery/pkg/types"

// NamespacedName is the name and namespace of the kubernetes object
// +k8s:openapi-gen=true
type NamespacedName struct {
	Name      string `json:"name,omitempty"`
	Namespace string `json:"namespace,omitempty"`
}

func NewNamespacedName(name types.NamespacedName) NamespacedName {
	return NamespacedName{
		Name:      name.Name,
		Namespace: name.Namespace,
	}
}

// OprStageAction represents the action type of controller reconcile stage
type OprStageAction string

const (
	StageActionApply  OprStageAction = "apply"
	StageActionDelete OprStageAction = "delete"
)

// OprStageStatus represents the status of controller stage
type OprStageStatus string

const (
	StageResultSucceeded OprStageStatus = "succeeded"
	StageResultFailed    OprStageStatus = "failed"
)

func (e *XlineCluster) ObjKey() types.NamespacedName {
	if e.objKey == nil {
		key := types.NamespacedName{Namespace: e.Namespace, Name: e.Name}
		e.objKey = &key
		return key
	} else {
		return *e.objKey
	}
}
