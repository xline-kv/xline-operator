package transformer

import (
	"fmt"
	"strings"

	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
	"github.com/xline-kv/xline-operator/internal/constants"
	"github.com/xline-kv/xline-operator/internal/util"
	appv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	rbacv1 "k8s.io/api/rbac/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	"k8s.io/utils/pointer"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
)

func GetXlineInstanceLabels(xlineClusterName types.NamespacedName) map[string]string {
	return MakeResourceLabels(xlineClusterName.Name)
}

func GetXlineDiscoveryLabels(xlineClusterName types.NamespacedName) map[string]string {
	return MakeResourceLabels(fmt.Sprintf("%s-discovery", xlineClusterName.Name))
}

func GetMemberTopology(cr *xapi.XlineCluster) string {
	replicas := int(cr.Spec.Replicas)
	members := make([]string, replicas)
	for i := 0; i < replicas; i++ {
		podName := fmt.Sprintf("%s-%d", cr.Name, i)
		dnsName := fmt.Sprintf("%s.%s.%s.svc.cluster.local", podName, cr.Name, cr.Namespace)
		members[i] = fmt.Sprintf("%s=%s:%d", podName, dnsName, constants.XlinePort)
	}
	return strings.Join(members, ",")
}

func getAuthInfo(auth_sec *xapi.XlineAuthSecret) ([]corev1.Volume, []corev1.VolumeMount, []corev1.EnvVar) {
	if auth_sec == nil {
		return []corev1.Volume{}, []corev1.VolumeMount{}, []corev1.EnvVar{}
	}
	return []corev1.Volume{
			{Name: "auth-cred", VolumeSource: corev1.VolumeSource{
				Secret: &corev1.SecretVolumeSource{
					SecretName: *auth_sec.Name,
				},
			}},
		}, []corev1.VolumeMount{
			{Name: "auth-cred", ReadOnly: true, MountPath: *auth_sec.MountPath},
		}, []corev1.EnvVar{
			{Name: "AuthPublicKey", Value: fmt.Sprintf("%s/%s", *auth_sec.MountPath, *auth_sec.PubKey)},
			{Name: "AuthPrivateKey", Value: fmt.Sprintf("%s/%s", *auth_sec.MountPath, *auth_sec.PriKey)},
		}
}

func getConfigInfo(cr *xapi.XlineCluster) []corev1.EnvFromSource {
	if cr.Spec.BootstrapArgs == nil {
		return []corev1.EnvFromSource{}
	}
	return []corev1.EnvFromSource{
		{ConfigMapRef: &corev1.ConfigMapEnvSource{
			LocalObjectReference: corev1.LocalObjectReference{
				Name: fmt.Sprintf("%s-config", cr.Name),
			},
		}},
	}
}

func MakeDiscoveryService(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.Service {
	svcLabel := GetXlineDiscoveryLabels(cr.ObjKey())
	service := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      fmt.Sprintf("%s-discovery", cr.Name),
			Namespace: cr.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Ports: []corev1.ServicePort{
				{
					Name: "discovery-port",
					Port: constants.DiscoveryPort,
				},
			},
			Selector: svcLabel,
		},
	}
	_ = controllerutil.SetOwnerReference(cr, service, scheme)
	return service
}

func MakeDiscoverySA(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.ServiceAccount {
	sa := &corev1.ServiceAccount{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline-discovery",
			Namespace: cr.Namespace,
			Labels:    GetXlineDiscoveryLabels(cr.ObjKey()),
		},
	}
	_ = controllerutil.SetOwnerReference(cr, sa, scheme)
	return sa
}

func MakeDiscoveryRole(cr *xapi.XlineCluster, scheme *runtime.Scheme) *rbacv1.Role {
	role := &rbacv1.Role{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline-discovery-role",
			Namespace: cr.Namespace,
			Labels:    GetXlineDiscoveryLabels(cr.ObjKey()),
		},
		Rules: []rbacv1.PolicyRule{
			{
				APIGroups: []string{corev1.GroupName},
				Resources: []string{"pods"},
				Verbs:     []string{"get", "list", "watch"},
			},
		},
	}

	_ = controllerutil.SetOwnerReference(cr, role, scheme)
	return role
}

func MakeDiscoveryRoleBinding(cr *xapi.XlineCluster, scheme *runtime.Scheme) *rbacv1.RoleBinding {
	rb := &rbacv1.RoleBinding{
		ObjectMeta: metav1.ObjectMeta{
			Name:      "xline-discovery-rolebinding",
			Namespace: cr.Namespace,
			Labels:    GetXlineDiscoveryLabels(cr.ObjKey()),
		},
		Subjects: []rbacv1.Subject{{
			Kind: rbacv1.ServiceAccountKind,
			Name: "xline-discovery",
		}},
		RoleRef: rbacv1.RoleRef{
			Kind:     "Role",
			Name:     "xline-discovery-role",
			APIGroup: rbacv1.GroupName,
		},
	}
	_ = controllerutil.SetOwnerReference(cr, rb, scheme)
	return rb
}

func MakeDiscoveryDeployment(cr *xapi.XlineCluster, scheme *runtime.Scheme, image string) *appv1.Deployment {
	discoveryLabel := GetXlineDiscoveryLabels(cr.ObjKey())
	podSpec := corev1.PodSpec{
		Containers: []corev1.Container{
			{
				Name:  "xline-discovery",
				Image: image,
				Command: []string{
					"/discovery",
				},
				Ports: []corev1.ContainerPort{
					{
						ContainerPort: constants.DiscoveryPort,
					},
				},
				Env: []corev1.EnvVar{
					{Name: "XC_NAME", Value: cr.Name},
					{Name: "NAMESPACE", Value: cr.Namespace},
				},
			},
		},
		ServiceAccountName: "xline-discovery",
	}

	deploy := &appv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:      cr.Name,
			Namespace: cr.Namespace,
		},
		Spec: appv1.DeploymentSpec{
			Replicas: pointer.Int32(1),
			Selector: &metav1.LabelSelector{
				MatchLabels: discoveryLabel,
			},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{
					Labels: discoveryLabel,
				},
				Spec: podSpec,
			},
		},
	}

	_ = controllerutil.SetOwnerReference(cr, deploy, scheme)
	return deploy
}

func MakeService(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.Service {
	svcLabel := GetXlineInstanceLabels(cr.ObjKey())
	service := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      cr.Name,
			Namespace: cr.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Ports: []corev1.ServicePort{
				{
					Name: "xline-port",
					Port: constants.XlinePort,
				},
			},
			Selector:  svcLabel,
			ClusterIP: "None",
		},
	}
	_ = controllerutil.SetOwnerReference(cr, service, scheme)
	return service
}

func MakeScriptCM(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.ConfigMap {
	cm := &corev1.ConfigMap{
		ObjectMeta: metav1.ObjectMeta{
			Name:      fmt.Sprintf("%s-script", cr.Name),
			Namespace: cr.Namespace,
			Labels:    GetXlineInstanceLabels(cr.ObjKey()),
		},
		Data: map[string]string{
			"startup-script": XlineStartScript,
		},
	}
	_ = controllerutil.SetOwnerReference(cr, cm, scheme)
	return cm
}

func MakeConfigMap(cr *xapi.XlineCluster, scheme *runtime.Scheme) *corev1.ConfigMap {
	cm := &corev1.ConfigMap{
		ObjectMeta: metav1.ObjectMeta{
			Name:      fmt.Sprintf("%s-config", cr.Name),
			Namespace: cr.Namespace,
			Labels:    GetXlineInstanceLabels(cr.ObjKey()),
		},
		Data: cr.Spec.BootArgs(),
	}
	_ = controllerutil.SetOwnerReference(cr, cm, scheme)
	return cm
}

func MakeStatefulSet(cr *xapi.XlineCluster, scheme *runtime.Scheme) *appv1.StatefulSet {
	crName := types.NamespacedName{Namespace: cr.Namespace, Name: cr.Name}
	stsLabels := GetXlineInstanceLabels(crName)

	envs := []corev1.EnvVar{
		{Name: "MEMBERS", Value: GetMemberTopology(cr)},
	}

	volumes := []corev1.Volume{
		{
			Name: "startup-script",
			VolumeSource: corev1.VolumeSource{
				ConfigMap: &corev1.ConfigMapVolumeSource{
					LocalObjectReference: corev1.LocalObjectReference{
						Name: fmt.Sprintf("%s-script", cr.Name),
					},
					Items: []corev1.KeyToPath{{Key: "startup-script", Path: "xline_start_script.sh"}},
				},
			},
		},
	}

	volumeMounts := []corev1.VolumeMount{
		{Name: "xline-storage", MountPath: DataDir},
		{Name: "startup-script", ReadOnly: true, MountPath: "/usr/local/script"},
	}

	authVol, authVM, authEnvs := getAuthInfo(cr.Spec.AuthSecrets)
	volumes = append(volumes, authVol...)
	volumeMounts = append(volumeMounts, authVM...)
	envs = append(envs, authEnvs...)

	pvcTemplates := []corev1.PersistentVolumeClaim{
		util.NewReadWriteOncePVC("xline-storage", cr.Spec.StorageClassName, cr.Spec.Requests.Storage()),
	}

	// pod template: main container
	mainContainer := corev1.Container{
		Name:            "xline",
		Image:           *cr.Spec.Image,
		ImagePullPolicy: cr.Spec.ImagePullPolicy,
		Ports: []corev1.ContainerPort{
			{Name: "xline-port", ContainerPort: constants.XlinePort},
		},
		Command:      []string{"/bin/bash", "/usr/local/script/xline_start_script.sh"},
		Env:          envs,
		EnvFrom:      getConfigInfo(cr),
		VolumeMounts: volumeMounts,
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
			Name:      cr.Name,
			Namespace: cr.Namespace,
			Labels:    stsLabels,
		},
		Spec: appv1.StatefulSetSpec{
			Replicas:             &cr.Spec.Replicas,
			ServiceName:          cr.Name,
			Selector:             &metav1.LabelSelector{MatchLabels: stsLabels},
			VolumeClaimTemplates: pvcTemplates,
			Template:             podTemplate,
		},
	}

	_ = controllerutil.SetOwnerReference(cr, statefulSet, scheme)
	_ = controllerutil.SetControllerReference(cr, statefulSet, scheme)
	return statefulSet
}
