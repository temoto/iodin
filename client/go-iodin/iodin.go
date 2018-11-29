package iodin

import (
	"os"

	"github.com/golang/protobuf/proto"
	"github.com/juju/errors"
)

const modName string = "iodin-client"

//go:generate protoc -I=../../protobuf --go_out=./ ../../protobuf/iodin.proto

type Client struct {
	proc *os.Process
	rf   *os.File
	wf   *os.File
}

func NewClient(path string) (*Client, error) {
	// one pipe to send data to iodin and one to receive
	fSendRead, fSendWrite, err := os.Pipe()
	if err != nil {
		return nil, errors.Trace(err)
	}
	fRecvRead, fRecvWrite, err := os.Pipe()
	if err != nil {
		return nil, errors.Trace(err)
	}

	attr := &os.ProcAttr{
		Env:   nil,
		Files: []*os.File{fSendRead, fRecvWrite, os.Stderr},
	}
	p, err := os.StartProcess(path, nil, attr)
	if err != nil {
		return nil, errors.Trace(err)
	}

	c := &Client{
		proc: p,
		rf:   fRecvRead,
		wf:   fSendWrite,
	}
	return c, nil
}

func (self *Client) Close() error {
	r := Request{
		Command: Request_STOP,
	}
	return self.Do(&r, new(Response))
}

func (self *Client) Do(request *Request, response *Response) error {
	// sock.SetDeadline(time.Now().Add(5*time.Second))
	// defer sock.SetDeadline(time.Time{})
	buf := make([]byte, 256)
	pb := proto.NewBuffer(buf[:0])
	{
		pb.EncodeFixed32(uint64(proto.Size(request)))
		err := pb.Marshal(request)
		if err != nil {
			return errors.Annotatef(err, "iodin.Do.Marshal req=%s", request.String())
		}
		_, err = self.wf.Write(pb.Bytes())
		if err != nil {
			return errors.Annotatef(err, "iodin.Do.Write req=%s", request.String())
		}
	}

	n, err := self.rf.Read(buf[:4])
	if err != nil || n < 4 {
		return errors.Annotatef(err, "iodin.Do.Read len buf=%x n=%d/4 req=%s", buf[:n], n, request.String())
	}
	pb.SetBuf(buf[:n])
	lu64, err := pb.DecodeFixed32()
	responseLen := int(lu64)
	if err != nil {
		return errors.Annotatef(err, "iodin.Do.Read len decode buf=%x req=%s", buf[:n], request.String())
	}
	if responseLen > len(buf) {
		return errors.Errorf("iodin.Do.Read buf overflow %d>%d req=%s", responseLen, len(buf), request.String())
	}
	n, err = self.rf.Read(buf[:responseLen])
	if err != nil {
		return errors.Annotatef(err, "iodin.Do.Read response buf=%x req=%s", buf[:n], request.String())
	}
	if n < responseLen {
		return errors.NotImplementedf("iodin.Do.Read response did not fit in one read() syscall len=%d req=%s", responseLen, request.String())
	}
	pb.SetBuf(buf[:n])
	err = pb.Unmarshal(response)
	if err != nil {
		return errors.Annotatef(err, "iodin.Do.Unmarshal buf=%x req=%s", pb.Bytes(), request.String())
	}
	return nil
}
