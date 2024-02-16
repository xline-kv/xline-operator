package server

import (
	"encoding/base64"
	"fmt"
	"io"
	"net/http"

	"github.com/emicklei/go-restful"
	"go.uber.org/zap"
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
}

type discovery struct{}

func NewDiscovery() Discovery {
	return &discovery{}
}

func (d *discovery) Discover(advertisePeerUrl string) (string, error) {
	return fmt.Sprintf("%s", advertisePeerUrl), nil
}

// NewServer creates a new server.
func NewServer() Server {
	s := &server{
		discovery: NewDiscovery(),
		container: restful.NewContainer(),
	}
	s.registerHandlers()
	return s
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
	encodedAdvertisePeerURL := req.PathParameter("advertise-peer-url")
	data, err := base64.StdEncoding.DecodeString(encodedAdvertisePeerURL)
	if err != nil {
		zap.S().Errorf("failed to decode advertise-peer-url: %s, register-type is: %s", encodedAdvertisePeerURL)
		if werr := resp.WriteError(http.StatusInternalServerError, err); werr != nil {
			zap.S().Errorf("failed to writeError: %v", werr)
		}
		return
	}
	advertisePeerURL := string(data)

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
