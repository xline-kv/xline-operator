# xline-operator

The xline-operator is a powerful tool designed to automate the process of bootstrapping, monitoring, snapshotting, and
recovering an xline cluster on Kubernetes.

## Getting Started

### Setup RBAC for xline operator

Xline operator requires a `ClusterRole` with permission to create Kubernetes Custom Resource Definition (CRD).

In this example, the `ClusterRole` specified in `examples/cluster-role.yml` will bind to the default service account
through `examples/cluster-role-binding.yml`.
You can change the binding if you want.

Apply RBAC:

```bash
$ kubectl apply -f examples/cluster-role.yml
clusterrole.rbac.authorization.k8s.io/xline-operator-role created

$ kubectl apply -f examples/cluster-role-binding.yml
clusterrolebinding.rbac.authorization.k8s.io/xline-operator-rolebinding created
```

Inspect RBAC:

```bash
$ kubectl describe clusterrole xline-operator-role
Name:         xline-operator-role
Labels:       <none>
Annotations:  <none>
PolicyRule:
  Resources                                       Non-Resource URLs  Resource Names  Verbs
  ---------                                       -----------------  --------------  -----
  endpoints                                       []                 []              [*]
  events                                          []                 []              [*]
  persistentvolumeclaims                          []                 []              [*]
  pods                                            []                 []              [*]
  services                                        []                 []              [*]
  customresourcedefinitions.apiextensions.k8s.io  []                 []              [*]
  statefulsets.apps                               []                 []              [*]
  cronjobs.batch                                  []                 []              [*]
  xlineclusters.xlineoperator.xline.cloud         []                 []              [*]
```

### Setup xline operator

Create a deployment:

```bash
$ kubectl apply -f examples/crd-deployment.yml
deployment.apps/my-xline-operator created
```

xline operator will automatically create a CRD:

```bash
$ kubectl get crd
NAME                                      CREATED AT
xlineclusters.xlineoperator.xline.cloud   -
```

### Create an xline cluster

Create an xline cluster:

```bash
$ kubectl apply -f examples/xline-cluster-example.yml
xlinecluster.xlineoperator.xline.cloud/my-xline-cluster created
```

Inspect xline pods:

```bash
$ kubectl get pods
NAME                                 READY   STATUS    RESTARTS        AGE
my-xline-cluster-0                   1/1     Running   0               -
my-xline-cluster-1                   1/1     Running   0               -
my-xline-cluster-2                   1/1     Running   0               -
```

### Resize the xline cluster

You can use `kubectl scale` to resize the replicas of xline servers. `xlinecluster` has a short name `xc`

```bash
$ kubectl scale xc my-xline-cluster --replicas=5
xlinecluster.xlineoperator.xline.cloud/my-xline-cluster scaled
```

Then we have 5 xline servers now!

```bash
$ kubectl get pods
NAME                                 READY   STATUS    RESTARTS      AGE
my-xline-cluster-0                   1/1     Running   0             -
my-xline-cluster-1                   1/1     Running   0             -
my-xline-cluster-2                   1/1     Running   0             -
my-xline-cluster-3                   1/1     Running   0             -
my-xline-cluster-4                   1/1     Running   0             -
```

Notice that you cannot scale your cluster size less than 3. The operator cannot work with less than 3 nodes.

```bash
$ kubectl scale xc my-xline-cluster --replicas=1
The XlineCluster "my-xline-cluster" is invalid: spec.size: Invalid value: 1: spec.size in body should be greater than or equal to 3
```

### Delete the xline cluster

```bash
$ kubectl delete -f examples/xline-cluster-example.yml
```

## Code of Conduct

Read the document [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for more details.
