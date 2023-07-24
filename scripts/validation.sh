#!/bin/bash

CLUSTER_NAME="my-xline-cluster"
DNS_SUFFIX="cluster.local"
NAMESPACE="default"
XLINE_PORT="2379"
ENDPOINTS=""

# start a tester pod to run etcdctl
kubectl delete pod/tester 2>/dev/null
kubectl wait --for=delete pod/tester --timeout=10m
kubectl run tester --image=registry.k8s.io/pause:3.1 --restart=Never
kubectl wait --for=condition=ready pod/tester --timeout=10m
kubectl debug tester --image=gcr.io/etcd-development/etcd:v3.5.5 --container=etcdctl --target=tester
# waiting for the debug request to be received by the API server.
sleep 5
kubectl wait --for=condition=containersready pod/tester --timeout=10m

# avoid mysterious "Error from server: error dialing backend: EOF" error
etcdctl() {
  for ((i = 0; i < 5; i++)); do
    if output=$(kubectl exec tester -c etcdctl -- etcdctl --endpoints="$ENDPOINTS" $@ 2>&1); then
      echo -e "$output"
      return
    fi
    sleep 1
  done
}

mk_endpoints() {
  ENDPOINTS="http://${CLUSTER_NAME}-0.${CLUSTER_NAME}.${NAMESPACE}.svc.${DNS_SUFFIX}:${XLINE_PORT}"
  for ((i = 1; i < $1; i++)); do
    ENDPOINTS="${ENDPOINTS},http://${CLUSTER_NAME}-${i}.${CLUSTER_NAME}.${NAMESPACE}.svc.${DNS_SUFFIX}:${XLINE_PORT}"
  done
  echo "endpoints: $ENDPOINTS"
}

scale_cluster() {
  kubectl scale xc $CLUSTER_NAME --replicas="$1"
  # TODO wait for xlinecluster resource status
  kubectl wait --for=jsonpath='{.status.updatedReplicas}'="$1" sts/$CLUSTER_NAME --timeout=10m
  # wait for the last container to be running
  kubectl wait --for=jsonpath='{.status.readyReplicas}'="$1" sts/$CLUSTER_NAME --timeout=10m
  got=$(kubectl get xc $CLUSTER_NAME -o=jsonpath='{.spec.size}')
  if [ "$got" -ne "$1" ]; then
    echo "failed scale cluster"
    echo "expect size: $1"
    echo "got size: $got"
    exit 1
  fi
}

run_expect() {
  got=$(etcdctl "$1")
  expect=$(echo -e "$2")
  if [ "${got//$'\r'/}" == "$expect" ]; then
    echo "command run success"
  else
    echo "command run failed"
    echo "command: etcdctl $1"
    echo "expect: $expect"
    echo "got: $got"
    exit 1
  fi
}

basic_validation() {
  echo "=== Basic Validation ==="
  mk_endpoints 3
  run_expect "put A 1" "OK"
  run_expect "get A" "A\n1"
}

scale_validation() {
  echo "=== Scale Validation ==="
  scale_cluster 5
  mk_endpoints 5
  run_expect "put A 1" "OK"
  run_expect "get A" "A\n1"
  scale_cluster 3
  mk_endpoints 3
  run_expect "put A 1" "OK"
  run_expect "get A" "A\n1"
}

basic_validation
scale_validation
