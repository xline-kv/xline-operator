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
