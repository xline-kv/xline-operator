package transformer

import (
	"fmt"
	"strings"

	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	"github.com/xline-kv/xline-operator/internal/util"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
)

func GetConfigMapKey(xlineClusterName types.NamespacedName) types.NamespacedName {
	return types.NamespacedName{
		Namespace: xlineClusterName.Namespace,
		Name:      fmt.Sprintf("%s-config", xlineClusterName.Name),
	}
}

func GetServiceKey(xlineClusterName types.NamespacedName) types.NamespacedName {
	return types.NamespacedName{
		Namespace: xlineClusterName.Namespace,
		Name:      fmt.Sprintf("%s-svc", xlineClusterName.Name),
	}
}

func GetStatefulSetKey(xlineClusterName types.NamespacedName) types.NamespacedName {
	return types.NamespacedName{
		Namespace: xlineClusterName.Namespace,
		Name:      fmt.Sprintf("%s-sts", xlineClusterName.Name),
	}
}

func GetXlineInstanceLabels(xlineClusterName types.NamespacedName) map[string]string {
	return MakeResourceLabels(xlineClusterName.Name)
}

func GetXlineImage(r *xapi.XlineCluster) string {
	version := r.Spec.Version
	return fmt.Sprintf("%s:%s", *r.Spec.Image, version)
}

func GetMemberTopology(stsRef types.NamespacedName, svcName string, replicas int) string {
	members := make([]string, replicas)
	for i := 0; i < replicas; i++ {
		podName := fmt.Sprintf("%s-%d", stsRef.Name, i)
		dnsName := fmt.Sprintf("%s.%s.%s.svc.cluster.local", podName, svcName, stsRef.Namespace)
		members[i] = fmt.Sprintf("%s=%s:2379", podName, dnsName)
	}
	return strings.Join(members, ",")
}

func defaultConfigMap(clusterName string) map[string]string {
	return map[string]string{
		"init-leader": fmt.Sprintf("%s-sts-0", clusterName),
		"log-level":   "info",
		"engine":      "rocksdb",
		"data-dir":    "/usr/local/xline/data-dir",
		"jwt-pubkey":  "",
		"jwt-prikey":  "",
	}
}

func MakeConfigMap(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.ConfigMap {
	defaultConf := defaultConfigMap(cr.ObjKey().Name)
	data := util.IntersectAndMergeMaps(cr.Spec.Config, defaultConf)
	configMapRef := GetConfigMapKey(cr.ObjKey())
	configMap := &corev1.ConfigMap{
		ObjectMeta: metav1.ObjectMeta{
			Name:      configMapRef.Name,
			Namespace: configMapRef.Namespace,
			Labels:    GetXlineInstanceLabels(cr.ObjKey()),
		},
		Data: data,
	}
	_ = controllerutil.SetOwnerReference(cr, configMap, scheme)
	return configMap
}

func MakeService(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.Service {
	svcRef := GetServiceKey(cr.ObjKey())
	svcLabel := GetXlineInstanceLabels(cr.ObjKey())
	service := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      svcRef.Name,
			Namespace: svcRef.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Ports: []corev1.ServicePort{
				{
					Name: "xline-port",
					Port: 2379,
				},
			},
			Selector:  svcLabel,
			ClusterIP: "None",
		},
	}
	_ = controllerutil.SetOwnerReference(cr, service, scheme)
	return service
}

func MakeStatefulSet(cr *xapi.XlineCluster, scheme *runtime.Scheme) *appv1.StatefulSet {
	crName := types.NamespacedName{Namespace: cr.Namespace, Name: cr.Name}
	configMapRef := GetConfigMapKey(cr.ObjKey())
	stsRef := GetStatefulSetKey(crName)
	stsLabels := GetXlineInstanceLabels(crName)
	svcName := GetServiceKey(cr.ObjKey()).Name

	// pod template: main container
	mainContainer := corev1.Container{
		Name:            "xline",
		Image:           GetXlineImage(cr),
		ImagePullPolicy: cr.Spec.ImagePullPolicy,
		Ports: []corev1.ContainerPort{
			{Name: "xline-port", ContainerPort: 2379},
		},
		Env: []corev1.EnvVar{
			{Name: "MEMBERS", Value: GetMemberTopology(stsRef, svcName, int(cr.Spec.Replicas))},
			{Name: "INIT_LEADER", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "init-leader")},
			{Name: "RUST_LOG", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "log-level")},
			{Name: "ENGINE", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "engine")},
			{Name: "DATA_DIR", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "data-dir")},
			{Name: "AUTH_PUBLIC_KEY", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "jwt-pubkey")},
			{Name: "AUTH_PRIVATE_KEY", ValueFrom: util.NewEnvVarConfigMapSource(configMapRef.Name, "jwt-prikey")},
		},
	}

	// pod template
	podTemplate := corev1.PodTemplateSpec{
		ObjectMeta: metav1.ObjectMeta{
			Labels: stsLabels,
		},
		Spec: corev1.PodSpec{
			Containers: []corev1.Container{mainContainer},
		},
	}

	// TODO: add an update strategy here

	// statefulset
	statefulSet := &appv1.StatefulSet{
		ObjectMeta: metav1.ObjectMeta{
			Name:      stsRef.Name,
			Namespace: stsRef.Namespace,
			Labels:    stsLabels,
		},
		Spec: appv1.StatefulSetSpec{
			Replicas:    &cr.Spec.Replicas,
			ServiceName: svcName,
			Selector:    &metav1.LabelSelector{MatchLabels: stsLabels},
			Template:    podTemplate,
		},
	}

	_ = controllerutil.SetOwnerReference(cr, statefulSet, scheme)
	_ = controllerutil.SetControllerReference(cr, statefulSet, scheme)
	return statefulSet
}
