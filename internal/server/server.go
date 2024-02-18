package server

import (
	"context"
	"encoding/base64"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/emicklei/go-restful"
	"go.uber.org/zap"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"
)

type Discovery interface {
	Discover(string) (string, error)
}

type Server interface {
	ListenAndServe(addr string)
}

type server struct {
	discovery Discovery
	container *restful.Container
	cli       *kubernetes.Clientset
	ns        string
	name      string
}

type discovery struct{}

func NewDiscovery() Discovery {
	return &discovery{}
}

func (d *discovery) Discover(advertisePeerUrl string) (string, error) {
	return advertisePeerUrl, nil
}

// NewServer creates a new server.
func NewServer(namespace string, name string) (Server, error) {
	config, err := rest.InClusterConfig()
	if err != nil {
		return nil, err
	}

	clientset, err := kubernetes.NewForConfig(config)
	if err != nil {
		return nil, err
	}

	s := &server{
		discovery: NewDiscovery(),
		container: restful.NewContainer(),
		cli:       clientset,
		ns:        namespace,
		name:      name,
	}
	s.registerHandlers()
	return s, nil
}

func (s *server) registerHandlers() {
	ws := new(restful.WebService)
	ws.Route(ws.GET("/new/{advertise-peer-url}").To(s.newHandler))
	s.container.Add(ws)
}

func (s *server) ListenAndServe(addr string) {
	zap.S().Fatal(http.ListenAndServe(addr, s.container.ServeMux))
}

func (s *server) newHandler(req *restful.Request, resp *restful.Response) {
	pods, err := s.cli.CoreV1().Pods("default").List(context.TODO(), metav1.ListOptions{
		LabelSelector: fmt.Sprintf("app.kubernetes.io/instance=%s", s.name),
	})
	if err != nil {
		zap.S().Errorf("failed to get xline running pod: %s", err)
		if werr := resp.WriteError(http.StatusInternalServerError, err); werr != nil {
			zap.S().Errorf("failed to writeError: %v", werr)
		}
		return
	}
	var runningPods []string
	for _, pod := range pods.Items {
		if pod.Status.Phase == corev1.PodRunning {
			runningPods = append(runningPods, pod.Spec.Hostname)
		}
	}
	encodedAdvertisePeerURL := req.PathParameter("advertise-peer-url")
	data, err := base64.StdEncoding.DecodeString(encodedAdvertisePeerURL)
	if err != nil {
		zap.S().Errorf("failed to decode advertise-peer-url: %s", encodedAdvertisePeerURL)
		if werr := resp.WriteError(http.StatusInternalServerError, err); werr != nil {
			zap.S().Errorf("failed to writeError: %v", werr)
		}
		return
	}
	// advertisePeerURL := string(data)
	advertisePeerURL := fmt.Sprintf("%s: %s", string(data), strings.Join(runningPods, ","))
	var result string
	result, err = s.discovery.Discover(advertisePeerURL)
	if err != nil {
		zap.S().Errorf("failed to discover: %s, %v", advertisePeerURL, err)
		if werr := resp.WriteError(http.StatusInternalServerError, err); werr != nil {
			zap.S().Errorf("failed to writeError: %v", werr)
		}
		return
	}

	zap.S().Infof("generated args for %s: %s", advertisePeerURL, result)
	if _, err := io.WriteString(resp, result); err != nil {
		zap.S().Errorf("failed to writeString: %s, %v", result, err)
	}
}
