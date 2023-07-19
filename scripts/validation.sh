#!/bin/bash

CONTAINER_NAME='my-xline'
CLUSTER_NAME='my-xline-cluster'
DNS_SUFFIX='cluster.local'
NAMESPACE='default'
XLINE_PORT='2379'
ENDPOINTS="http://${CLUSTER_NAME}-0.${CLUSTER_NAME}.${NAMESPACE}.svc.${DNS_SUFFIX}:${XLINE_PORT},http://${CLUSTER_NAME}-1.${CLUSTER_NAME}.${NAMESPACE}.svc.${DNS_SUFFIX}:${XLINE_PORT},http://${CLUSTER_NAME}-2.${CLUSTER_NAME}.${NAMESPACE}.svc.${DNS_SUFFIX}:${XLINE_PORT}"

# install etcdctl in pod `my-xline-cluster-0`
kubectl exec -it pod/my-xline-cluster-0 -c $CONTAINER_NAME -- bash -c "apt update && apt install -y etcd"

etcdctl() {
  kubectl exec -it pod/my-xline-cluster-0 -c $CONTAINER_NAME -- bash -c "ETCDCTL_API=3 etcdctl --endpoints='${ENDPOINTS}' ${1}"
}

run_expect() {
  got=$(etcdctl "${1}")
  expect=$(echo -e "${2}")
  if [ "${got//$'\r'/}" == "${expect}" ]; then
    echo "command run success"
  else
    echo "command run failed"
    echo "command: etcdctl ${1}"
    echo "expect: ${expect}"
    echo "got: ${got}"
    exit 1
  fi
}

basic_validation() {
  run_expect "put A 1" "OK"
  run_expect "get A" "A\n1"
}

basic_validation
