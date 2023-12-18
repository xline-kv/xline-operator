package v1alpha1

import (
	"testing"

	"github.com/stretchr/testify/assert"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
)

func TestObjKey(t *testing.T) {
	xlineCluster := XlineCluster{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline",
			Namespace: "default",
		},
	}
	expected_objkey := types.NamespacedName{Name: "xline", Namespace: "default"}
	assert.Equal(t, xlineCluster.ObjKey(), expected_objkey)
}
