package transformer

import (
	"fmt"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

func TestXlineClusterFunc(t *testing.T) {
	test_image := "xline-img"
	test_image_version := "latest"
	xlineCluster := xapi.XlineCluster{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline",
			Namespace: "default",
		},
		Spec: xapi.XlineClusterSpec{
			Version:  test_image_version,
			Image:    &test_image,
			Replicas: 3,
		},
	}

	t.Run("GetServiceKey should work properly", func(t *testing.T) {
		xcLookupKey := xlineCluster.ObjKey()
		svcObj := GetServiceKey(xcLookupKey)
		assert.Equal(t, svcObj.Namespace, "default")
		assert.Equal(t, svcObj.Name, "xline-svc")
	})

	t.Run("GetStatefulSetKey should work properly", func(t *testing.T) {
		xcLookupKey := xlineCluster.ObjKey()
		stsObj := GetStatefulSetKey(xcLookupKey)
		assert.Equal(t, stsObj.Namespace, "default")
		assert.Equal(t, stsObj.Name, "xline-sts")
	})

	t.Run("GetXlineImage should work properly", func(t *testing.T) {
		xline_image := GetXlineImage(&xlineCluster)
		assert.Equal(t, xline_image, "xline-img:latest")
	})

	t.Run("GetMemberTopology should work properly", func(t *testing.T) {
		xcLookupKey := xlineCluster.ObjKey()
		stsRef := GetStatefulSetKey(xcLookupKey)
		svcName := GetServiceKey(xcLookupKey).Name
		topology := GetMemberTopology(stsRef, svcName, 3)
		topologyVec := strings.Split(topology, ",")
		assert.Equal(t, len(topologyVec), 3)
		for i := 0; i < 3; i++ {
			expectRes := fmt.Sprintf("xline-sts-%d=xline-sts-%d.xline-svc.default.svc.cluster.local:2379", i, i)
			assert.Equal(t, topologyVec[i], expectRes)
		}
	})
}
