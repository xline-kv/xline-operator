package util

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"k8s.io/apimachinery/pkg/types"
)

func TestK8sObjKeyStr(t *testing.T) {
	objkey := types.NamespacedName{Name: "xline", Namespace: "default"}
	assert.Equal(t, K8sObjKeyStr(objkey), "xline.default")
}

func TestIntersectAndMergeMaps(t *testing.T) {
	m1 := map[string]string{}
	m2 := map[string]string{"hello": "world"}
	assert.Equal(t, IntersectAndMergeMaps(m1, m2), m2)
	assert.Equal(t, IntersectAndMergeMaps(nil, m2), m2)

	m1 = map[string]string{"hello": "Sun"}
	m2 = map[string]string{"hello": "Earth", "world": "Moon"}
	assert.Equal(t, IntersectAndMergeMaps(m1, m2), map[string]string{"hello": "Sun", "world": "Moon"})

	assert.Equal(t, IntersectAndMergeMaps(m1, nil), map[string]string(nil))
	assert.Equal(t, IntersectAndMergeMaps(m1, map[string]string{}), map[string]string{})

}
