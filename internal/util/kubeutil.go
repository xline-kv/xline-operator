package util

import (
	"crypto/md5"
	"encoding/json"
	"fmt"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
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

func NewReadWriteOncePVC(name string, storageClassName *string, storageRequest *resource.Quantity) corev1.PersistentVolumeClaim {
	pvc := corev1.PersistentVolumeClaim{
		ObjectMeta: metav1.ObjectMeta{
			Name: name,
		},
		Spec: corev1.PersistentVolumeClaimSpec{
			AccessModes:      []corev1.PersistentVolumeAccessMode{corev1.ReadWriteOnce},
			StorageClassName: storageClassName,
		},
	}
	if storageRequest != nil {
		pvc.Spec.Resources.Requests = corev1.ResourceList{corev1.ResourceStorage: *storageRequest}
	}
	return pvc
}
