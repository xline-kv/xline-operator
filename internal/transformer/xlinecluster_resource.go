package transformer

import (
	"fmt"
	"strings"

	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
)

const (
	XlinePort = 2379
	DataDir   = "/usr/local/xline/data-dir"
)

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

func GetMemberTopology(stsRef types.NamespacedName, svcName string, replicas int) string {
	members := make([]string, replicas)
	for i := 0; i < replicas; i++ {
		podName := fmt.Sprintf("%s-%d", stsRef.Name, i)
		dnsName := fmt.Sprintf("%s.%s.%s.svc.cluster.local", podName, svcName, stsRef.Namespace)
		members[i] = fmt.Sprintf("%s=%s:%d", podName, dnsName, XlinePort)
	}
	return strings.Join(members, ",")
}

func GetAuthSecretVolume(auth_sec *xapi.XlineAuthSecret) []corev1.Volume {
	if auth_sec == nil {
		return []corev1.Volume{}
	}
	return []corev1.Volume{
		{Name: "auth-cred", VolumeSource: corev1.VolumeSource{
			Secret: &corev1.SecretVolumeSource{
				SecretName: *auth_sec.Name,
			},
		}},
	}
}

func GetAuthSecretVolumeMount(auth_sec *xapi.XlineAuthSecret) []corev1.VolumeMount {
	if auth_sec == nil {
		return []corev1.VolumeMount{}
	}
	return []corev1.VolumeMount{
		{Name: "auth-cred", ReadOnly: true, MountPath: *auth_sec.MountPath},
	}
}

func GetAuthSecretEnvVars(auth_sec *xapi.XlineAuthSecret) []corev1.EnvVar {
	if auth_sec == nil {
		return []corev1.EnvVar{}
	}
	return []corev1.EnvVar{
		{Name: "AUTH_PUBLIC_KEY", Value: fmt.Sprintf("%s/%s", *auth_sec.MountPath, *auth_sec.PubKey)},
		{Name: "AUTH_PRIVATE_KEY", Value: fmt.Sprintf("%s/%s", *auth_sec.MountPath, *auth_sec.PriKey)},
	}
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
					Port: XlinePort,
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
	stsRef := GetStatefulSetKey(crName)
	stsLabels := GetXlineInstanceLabels(crName)
	svcName := GetServiceKey(cr.ObjKey()).Name

	initCmd := []string{
		"xline",
		"--name", "$(POD_NAME)",
		"--members", "$(MEMBERS)",
		"--storage-engine", "rocksdb",
		"--data-dir", DataDir,
	}
	initCmd = append(initCmd, cr.Spec.BootArgs()...)

	envs := []corev1.EnvVar{
		{Name: "MEMBERS", Value: GetMemberTopology(stsRef, svcName, int(cr.Spec.Replicas))},
		{Name: "POD_NAME", ValueFrom: &corev1.EnvVarSource{
			FieldRef: &corev1.ObjectFieldSelector{
				FieldPath: "metadata.name",
			},
		}},
	}
	envs = append(envs, GetAuthSecretEnvVars(cr.Spec.AuthSecrets)...)

	volumes := GetAuthSecretVolume(cr.Spec.AuthSecrets)
	volumeMount := GetAuthSecretVolumeMount(cr.Spec.AuthSecrets)

	// pod template: main container
	mainContainer := corev1.Container{
		Name:            "xline",
		Image:           *cr.Spec.Image,
		ImagePullPolicy: cr.Spec.ImagePullPolicy,
		Ports: []corev1.ContainerPort{
			{Name: "xline-port", ContainerPort: XlinePort},
		},
		Command:      initCmd,
		Env:          envs,
		VolumeMounts: volumeMount,
	}

	// pod template
	podTemplate := corev1.PodTemplateSpec{
		ObjectMeta: metav1.ObjectMeta{
			Labels: stsLabels,
		},
		Spec: corev1.PodSpec{
			Volumes:    volumes,
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
