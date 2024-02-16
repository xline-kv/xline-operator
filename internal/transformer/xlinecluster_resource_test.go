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
	testImage := "xline-img:latest"
	xlineCluster := xapi.XlineCluster{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline",
			Namespace: "default",
		},
		Spec: xapi.XlineClusterSpec{
			Image:    &testImage,
			Replicas: 3,
		},
	}

	t.Run("GetXlineImage should work properly", func(t *testing.T) {
		xlineImage := *xlineCluster.Spec.Image
		assert.Equal(t, xlineImage, "xline-img:latest")
	})

	t.Run("GetMemberTopology should work properly", func(t *testing.T) {
		topology := GetMemberTopology(&xlineCluster)
		topologyVec := strings.Split(topology, ",")
		assert.Equal(t, len(topologyVec), 3)
		for i := 0; i < 3; i++ {
			expectRes := fmt.Sprintf("xline-%d=xline-%d.xline.default.svc.cluster.local:2379", i, i)
			assert.Equal(t, topologyVec[i], expectRes)
		}
	})
}
