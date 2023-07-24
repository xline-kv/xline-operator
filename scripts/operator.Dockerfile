FROM ubuntu:latest

COPY xline-operator /usr/local/bin

CMD ["/usr/local/bin/xline-operator"]
