#!/bin/bash
apt-get install -y make expect libssl-dev

# install minikube
curl -Lo minikube https://storage.googleapis.com/minikube/releases/latest/minikube-linux-amd64
install -m 755 minikube /usr/local/bin/minikube
