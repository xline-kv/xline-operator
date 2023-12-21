package util

import (
	"crypto/md5"
	"encoding/json"
	"fmt"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/types"
)

func K8sObjKeyStr(key types.NamespacedName) string {
	return fmt.Sprintf("%s.%s", key.Name, key.Namespace)
}

// Md5Hash returns the md5 hash of the given object base on json marshal.
func Md5Hash(obj any) (string, error) {
	if obj == nil {
		return "", nil
	}
	bytes, err := json.Marshal(obj)
	if err != nil {
		return "", err
	}
	hashStr := fmt.Sprintf("%x", md5.Sum(bytes))
	return hashStr, nil
}

// Md5HashOr returns the md5 hash of the given object base on json marshal.
// when error occurs, return the fallback string.
func Md5HashOr(obj any, fallback string) string {
	hash, err := Md5Hash(obj)
	if err != nil {
		return fallback
	}
	return hash
}

// IntersectAndMergeMaps patch the m1 on the m2
func IntersectAndMergeMaps[K comparable, V any](m1, m2 map[K]V) map[K]V {
	if len(m1) == 0 || m1 == nil || len(m2) == 0 || m2 == nil {
		return m2
	}
	for k := range m2 {
		if value, ok := m1[k]; ok {
			m2[k] = value
		}
	}
	return m2
}

func NewEnvVarConfigMapSource(cmName string, key string) *corev1.EnvVarSource {
	return &corev1.EnvVarSource{
		ConfigMapKeyRef: &corev1.ConfigMapKeySelector{
			LocalObjectReference: corev1.LocalObjectReference{Name: cmName},
			Key:                  key,
		},
	}
}
