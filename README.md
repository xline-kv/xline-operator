# xline-operator

The xline-operator is a powerful tool designed to automate the process of bootstrapping, monitoring, snapshotting, and
recovering an xline cluster on Kubernetes.

## Getting Started

### Install xline operator

Install the latest version of Xline Operator:

```bash
$ kubectl apply -f examples/xline-operator.yaml
```

Check the installation status:

```bash
# Check the CRDs
$ kubectl get crds
NAME                                   CREATED AT
xlineclusters.xline.io.datenlord.com   2024-01-12T12:30:46Z

# Check the controller Pod status
$  kubectl -n xline-operator-system get pods
NAME                                                 READY   STATUS    RESTARTS   AGE
xline-operator-controller-manager-5c9d5f6bc4-ndqzq   2/2     Running   0          98s
```

xline operator will automatically create a CRD:

```bash
$ kubectl get crd
NAME                                      CREATED AT
xlineclusters.xlineoperator.xline.cloud   -
```

### Create an Xline cluster

Follow the steps below to create an Xline cluster in your Kubernetes cluster:

```bash
# Apply xline-cluster.yaml to your Kubernetes cluster
$ kubectl apply -f examples/xline-cluster.yaml
xlinecluster.xline.io.datenlord.com/my-xline-cluster created
```

Note: the Xline cluster will be created in the `default` namespace by default. If you want to create it in another namespace, please modify the metadata.namespace field in the manifest YAML file or use the --namespace option.

Inspect xline pods:

```bash
# Get xline cluster info
$ kubectl get xlinecluster
NAME               AGE
my-xline-cluster   -

# Get Xline pod
$ kubectl get pods
NAME                     READY   STATUS    RESTARTS   AGE
my-xline-cluster-0   1/1     Running   0          -
my-xline-cluster-1   1/1     Running   0          -
my-xline-cluster-2   1/1     Running   0          -
```

### Delete the xline cluster

```bash
$ kubectl delete -f examples/xline-cluster.yml
```

## Code of Conduct

Read the document [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for more details.
