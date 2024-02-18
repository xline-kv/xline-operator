package main

import (
	"context"
	"flag"
	"fmt"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/signal"
	"syscall"

	"github.com/xline-kv/xline-operator/internal/server"

	"go.uber.org/zap"
)

var port int

func init() {
	zap.ReplaceGlobals(zap.Must(zap.NewProduction()))
	flag.IntVar(&port, "port", 10086, "The port that the xline discovery's http service runs on (default 10086)")
	flag.Parse()
}

// discovery_url="my-xline-cluster-discovery.default.svc:10086"
// domain="my-xline-cluster-0.my-xline-cluster.default.svc.cluster.local"
// encoded_domain_url=`echo ${domain}:2380 | base64 | tr "\n" " " | sed "s/ //g"`
// wget -qO- -T 3 http://${discovery_url}/new/${encoded_domain_url}

func main() {
	flag.CommandLine.VisitAll(func(flag *flag.Flag) {
		zap.S().Info("FLAG: --%s=%q", flag.Name, flag.Value)
	})

	xcName := os.Getenv("XC_NAME")
	if len(xcName) < 1 {
		zap.S().Fatal("ENV XC_NAME is not set")
	}

	ns := os.Getenv("NAMESPACE")
	if len(ns) < 1 {
		zap.S().Fatal("ENV NAMESPACE is not set")
	}

	go func() {
		addr := fmt.Sprintf("0.0.0.0:%d", port)
		zap.S().Infof("starting Xline Discovery server, listening on %s", addr)
		discoveryServer, err := server.NewServer(ns, xcName)
		if err != nil {
			zap.S().Fatal("cannot create k8s client: %s", err)
		}
		discoveryServer.ListenAndServe(addr)
	}()

	srv := http.Server{Addr: ":6060"}
	sc := make(chan os.Signal, 1)
	signal.Notify(sc,
		syscall.SIGHUP,
		syscall.SIGINT,
		syscall.SIGTERM,
		syscall.SIGQUIT,
	)

	go func() {
		sig := <-sc
		zap.S().Infof("got signal %s to exit", sig)
		if err2 := srv.Shutdown(context.Background()); err2 != nil {
			zap.S().Fatal("fail to shutdown the HTTP server", err2)
		}
	}()

	if err := srv.ListenAndServe(); err != http.ErrServerClosed {
		zap.S().Fatal(err)
	}
	zap.S().Infof("xline-discovery exited!!")
}
