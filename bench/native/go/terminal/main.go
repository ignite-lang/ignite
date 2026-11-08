// terminald of examples/terminal/terminal.ignite in Go: the standard library only, one
// goroutine per connection; a reference program for docs/quality.md row 3.3.
//
// The same bytes on the wire as the Ignite program for every input the benchmark and its
// functional check send: the prompt "terminal> " followed by IAC GA, the console's commands
// (help, echo <text>, status, quit, a note for an unknown one, nothing for a blank line),
// each reply and the next prompt in one write; CR LF, CR NUL, a lone CR and a lone LF end a
// line; DO is refused with WONT and WILL with DONT; AYT is answered "[yes]" and IAC GA; IP, a
// line over 4096 bytes and a subnegotiation over 1024 bytes end the session; a client silent
// for 10 minutes is dropped (a read deadline set before each read). Not implemented: the
// Synch (TCP urgent data and the Data Mark), which costs the Ignite server one SIOCATMARK per
// read.
// --port P (default 2323) on 127.0.0.1; "listening on 127.0.0.1:P" on stderr once bound.
package main

import (
	"bytes"
	"flag"
	"fmt"
	"net"
	"os"
	"time"
)

const (
	maximumLine           = 4096
	maximumSubnegotiation = 1024
	iac                   = 255
	dont                  = 254
	do                    = 253
	wont                  = 252
	will                  = 251
	sb                    = 250
	ga                    = 249
	ayt                   = 246
	ip                    = 244
	se                    = 240
)

var prompt = []byte("terminal> \xff\xf9")

const (
	ground = iota
	carriageReturn
	interpret
	negotiation
	subnegotiation
	subnegotiationInterpret
)

func isSpace(b byte) bool {
	return b == ' ' || b == '\t' || b == '\n' || b == '\r' || b == 0x0b || b == 0x0c
}

// respond gives the console's reply lines to one input line and whether it closes the session.
func respond(line []byte) ([][]byte, bool) {
	trimmed := bytes.TrimFunc(line, func(r rune) bool { return r < 0x80 && isSpace(byte(r)) })
	if len(trimmed) == 0 {
		return nil, false
	}
	end := bytes.IndexFunc(trimmed, func(r rune) bool { return r < 0x80 && isSpace(byte(r)) })
	if end < 0 {
		end = len(trimmed)
	}
	word := trimmed[:end]
	switch string(word) {
	case "help":
		return [][]byte{[]byte("help"), []byte("echo <text>"), []byte("status"), []byte("quit")}, false
	case "status":
		return [][]byte{[]byte("ready")}, false
	case "quit":
		return [][]byte{[]byte("bye")}, true
	case "echo":
		rest := end + 1
		if rest > len(trimmed) {
			rest = len(trimmed)
		}
		return [][]byte{trimmed[rest:]}, false
	}
	return [][]byte{append([]byte("unknown command: "), word...)}, false
}

// render appends each reply line as NVT data (LF as CR LF, a lone CR as CR NUL, IAC doubled)
// and CR LF.
func render(lines [][]byte, out []byte) []byte {
	for _, line := range lines {
		for i := 0; i < len(line); i++ {
			switch b := line[i]; b {
			case '\n':
				out = append(out, '\r', '\n')
			case '\r':
				if i+1 >= len(line) || line[i+1] != '\n' {
					out = append(out, '\r', 0)
				}
			case iac:
				out = append(out, iac, iac)
			default:
				out = append(out, b)
			}
		}
		out = append(out, '\r', '\n')
	}
	return out
}

func session(conn net.Conn) error {
	defer conn.Close()
	if _, err := conn.Write(prompt); err != nil {
		return err
	}
	buffer := make([]byte, 4096)
	line := make([]byte, 0, 256)
	out := make([]byte, 0, 4096)
	state, verb, count := ground, byte(0), 0
	for {
		conn.SetReadDeadline(time.Now().Add(10 * time.Minute))
		n, err := conn.Read(buffer)
		if n == 0 && err != nil {
			return nil
		}
		for _, b := range buffer[:n] {
			literal, haveLiteral, endOfLine := byte(0), false, false
			if state == carriageReturn {
				if b == '\n' || b == 0 {
					state = ground
					continue
				}
				state = ground
			}
			switch state {
			case ground:
				switch b {
				case iac:
					state = interpret
				case '\r':
					state, endOfLine = carriageReturn, true
				case '\n':
					endOfLine = true
				default:
					literal, haveLiteral = b, true
				}
			case interpret:
				switch {
				case b == iac:
					state, literal, haveLiteral = ground, iac, true
				case b >= will && b <= dont:
					state, verb = negotiation, b
				case b == sb:
					state, count = subnegotiation, 0
				case b == ayt:
					state = ground
					if _, err := conn.Write([]byte{'[', 'y', 'e', 's', ']', '\r', '\n', iac, ga}); err != nil {
						return err
					}
				case b == ip:
					return nil
				default:
					state = ground
				}
			case negotiation:
				state = ground
				var answer []byte
				if verb == do {
					answer = []byte{iac, wont, b}
				} else if verb == will {
					answer = []byte{iac, dont, b}
				}
				if answer != nil {
					if _, err := conn.Write(answer); err != nil {
						return err
					}
				}
			case subnegotiation:
				if count+1 > maximumSubnegotiation {
					return nil
				}
				count++
				if b == iac {
					state = subnegotiationInterpret
				}
			case subnegotiationInterpret:
				if b == se {
					state = ground
				} else {
					state = subnegotiation
				}
			}
			if haveLiteral {
				line = append(line, literal)
				if len(line) > maximumLine {
					conn.Write([]byte("line too long\r\n"))
					return nil
				}
			}
			if endOfLine {
				reply, closing := respond(line)
				line = line[:0]
				out = render(reply, out[:0])
				if closing {
					_, err := conn.Write(out)
					return err
				}
				out = append(out, prompt...)
				if _, err := conn.Write(out); err != nil {
					return err
				}
			}
		}
		if err != nil {
			return nil
		}
	}
}

func main() {
	port := flag.Int("port", 2323, "port on 127.0.0.1")
	flag.Parse()
	listener, err := net.Listen("tcp", fmt.Sprintf("127.0.0.1:%d", *port))
	if err != nil {
		fmt.Fprintf(os.Stderr, "terminal: %v\n", err)
		os.Exit(2)
	}
	fmt.Fprintf(os.Stderr, "listening on %s\n", listener.Addr())
	for {
		conn, err := listener.Accept()
		if err != nil {
			continue
		}
		go func() {
			if err := session(conn); err != nil {
				fmt.Fprintf(os.Stderr, "session ended with error: %v\n", err)
			}
		}()
	}
}
