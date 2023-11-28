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

package controller

import (
	"context"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
)

type ExpectClusterStatus struct {
	Stage          xapi.XlineClusterOprStage
	StageStatus    xapi.OprStageStatus
	Image          string
	StatefulSetRef xapi.NamespacedName
	ServiceRef     xapi.NamespacedName
}

func xlineNamespaceName(name string, ns string) types.NamespacedName {
	return types.NamespacedName{Name: name, Namespace: ns}
}

// +kubebuilder:docs-gen:collapse=Imports

var _ = Describe("XlineCluster controller", func() {

	// Define utility constants for object names and testing timeouts/durations and intervals.
	const (
		XlineClusterName      = "test-xline-cluster"
		XlineClusterNamespace = "default"
		XlineClusterStsName   = "test-xline-cluster-sts"
		XlineClusterSvcName   = "test-xline-cluster-svc"

		timeout  = time.Second * 10
		duration = time.Second * 10
		interval = time.Millisecond * 250
	)

	Context("When updating XlineCluster Status", func() {
		It("Should increase XlineCluster Status count when a new XlineCluster resource is created", func() {
			By("By creating a new XlineCluster")
			ctx := context.Background()
			image := "test-image"
			version := "latest"
			full_image := fmt.Sprintf("%s:%s", image, version)
			xlineCluster := &xapi.XlineCluster{
				TypeMeta: metav1.TypeMeta{
					APIVersion: "xline.kvstore.datenlord.com/v1alpha1",
					Kind:       "XlineCluster",
				},
				ObjectMeta: metav1.ObjectMeta{
					Name:      XlineClusterName,
					Namespace: XlineClusterNamespace,
				},
				Spec: xapi.XlineClusterSpec{
					Version:  version,
					Image:    &image,
					Replicas: 3,
				},
			}
			Expect(k8sClient.Create(ctx, xlineCluster)).Should(Succeed())

			xcLookupKey := xlineNamespaceName(XlineClusterName, XlineClusterNamespace)
			createdXlineCluster := &xapi.XlineCluster{}

			// We'll need to retry getting this newly created CronJob, given that creation may not immediately happen.
			Eventually(func() bool {
				err := k8sClient.Get(ctx, xcLookupKey, createdXlineCluster)
				return err == nil
			}, timeout, interval).Should(BeTrue())
			// Let's make sure our Schedule string value was properly converted/handled.
			Expect(createdXlineCluster.Spec.Replicas).Should(Equal(int32(3)))
			Expect(createdXlineCluster.Spec.Version).Should(Equal(version))
			Expect(*createdXlineCluster.Spec.Image).Should(Equal(image))

			By("XlinCluster Status should be updated")
			expected_status := ExpectClusterStatus{
				Stage:          xapi.StageComplete,
				StageStatus:    xapi.StageResultSucceeded,
				Image:          full_image,
				StatefulSetRef: xapi.NewNamespacedName(xlineNamespaceName(XlineClusterStsName, XlineClusterNamespace)),
				ServiceRef:     xapi.NewNamespacedName(xlineNamespaceName(XlineClusterSvcName, XlineClusterNamespace)),
			}

			Eventually(func() (ExpectClusterStatus, error) {
				err := k8sClient.Get(ctx, xcLookupKey, createdXlineCluster)
				if err != nil {
					return ExpectClusterStatus{}, err
				}
				real_status := ExpectClusterStatus{
					Stage:          createdXlineCluster.Status.Stage,
					StageStatus:    createdXlineCluster.Status.StageStatus,
					Image:          createdXlineCluster.Status.Image,
					StatefulSetRef: createdXlineCluster.Status.StatefulSetRef,
					ServiceRef:     createdXlineCluster.Status.ServiceRef,
				}
				return real_status, nil
			}, timeout, interval).Should(Equal(expected_status))
		})
	})

})
