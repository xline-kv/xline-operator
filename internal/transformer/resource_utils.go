package transformer

const (
	K8sNameLabelKey     = "app.kubernetes.io/name"
	K8sInstanceLabelKey = "app.kubernetes.io/instance"

	XlineK8sNameLabelValue      = "xline-cluster"
	XlineK8sManagedByLabelValue = "xline-operator"
)

// MakeResourceLabels make the k8s label meta for the managed resource
func MakeResourceLabels(xlineName string) map[string]string {
	labels := map[string]string{
		K8sInstanceLabelKey: xlineName,
	}
	return labels
}
