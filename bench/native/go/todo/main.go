// The todo service of examples/todo/todo.ignite in Go: net/http of the standard library
// (HTTP/1.1, keep-alive), one goroutine per connection; a reference program for
// docs/quality.md row 3.3.
//
// Same routes, status codes, bodies and Content-Types as the Ignite program, and the same
// durability: every change writes <shard>/<id>/<version>.json under todo-data in the working
// directory (a temporary file renamed over it, no fsync), then removes the older versions of
// that todo, before it answers; a deletion is a tombstone. Differences: the data is loaded
// once at start instead of on the first request (a failed load exits instead of answering
// 503), the todos are a map behind one sync.Mutex instead of a Store, JSON bodies are read
// with encoding/json (a repeated member: the last wins, Ignite takes the first), and headers
// are net/http's (a Date header, no Connection: keep-alive).
// --port P (default 8080) on 127.0.0.1; "listening on 127.0.0.1:P" on stderr once bound.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
)

const shardSize = 65536
const bodyLimit = 1 << 20

type todo struct {
	id      int64
	title   string
	done    bool
	version int64
	deleted bool
}

type state struct {
	mu     sync.Mutex
	todos  map[int64]*todo
	nextID int64
	data   string
	writes atomic.Uint64
}

func quote(text string, out *strings.Builder) {
	out.WriteByte('"')
	for _, c := range []byte(text) {
		switch c {
		case '"':
			out.WriteString(`\"`)
		case '\\':
			out.WriteString(`\\`)
		case '\n':
			out.WriteString(`\n`)
		case '\r':
			out.WriteString(`\r`)
		case '\t':
			out.WriteString(`\t`)
		case 8:
			out.WriteString(`\b`)
		case 12:
			out.WriteString(`\f`)
		default:
			if c < 0x20 {
				fmt.Fprintf(out, `\u%04x`, c)
			} else {
				out.WriteByte(c)
			}
		}
	}
	out.WriteByte('"')
}

func toJSON(t *todo, out *strings.Builder) {
	out.WriteString(`{"id":`)
	out.WriteString(strconv.FormatInt(t.id, 10))
	out.WriteString(`,"title":`)
	quote(t.title, out)
	out.WriteString(`,"done":`)
	out.WriteString(strconv.FormatBool(t.done))
	out.WriteByte('}')
}

func toRecord(t *todo) string {
	var out strings.Builder
	out.WriteString(`{"id":`)
	out.WriteString(strconv.FormatInt(t.id, 10))
	out.WriteString(`,"title":`)
	quote(t.title, &out)
	fmt.Fprintf(&out, `,"done":%t,"version":%d,"deleted":%t}`, t.done, t.version, t.deleted)
	return out.String()
}

// parseInt is Int.parse of Ignite: decimal digits with an optional leading '-'.
func parseInt(text string) (int64, bool) {
	digits := strings.TrimPrefix(text, "-")
	if digits == "" {
		return 0, false
	}
	for _, c := range []byte(digits) {
		if c < '0' || c > '9' {
			return 0, false
		}
	}
	n, err := strconv.ParseInt(text, 10, 64)
	return n, err == nil
}

func shardOf(id int64) string {
	if id <= shardSize {
		return "todos"
	}
	return "todos-" + strconv.FormatInt((id-1)/shardSize, 10)
}

func isShard(name string) bool {
	if name == "todos" {
		return true
	}
	rest, ok := strings.CutPrefix(name, "todos-")
	if !ok {
		return false
	}
	_, ok = parseInt(rest)
	return ok
}

func versionOf(name string) (int64, bool) {
	digits, ok := strings.CutSuffix(name, ".json")
	if !ok {
		return 0, false
	}
	return parseInt(digits)
}

func (s *state) save(t todo) error {
	dir := filepath.Join(s.data, shardOf(t.id), strconv.FormatInt(t.id, 10))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	n := s.writes.Add(1)
	temporary := filepath.Join(dir, fmt.Sprintf(".tmp-%d-%d", os.Getpid(), n))
	if err := os.WriteFile(temporary, []byte(toRecord(&t)), 0o644); err != nil {
		os.Remove(temporary)
		return err
	}
	if err := os.Rename(temporary, filepath.Join(dir, strconv.FormatInt(t.version, 10)+".json")); err != nil {
		os.Remove(temporary)
		return err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if v, ok := versionOf(entry.Name()); ok && v < t.version {
			os.Remove(filepath.Join(dir, entry.Name()))
		}
	}
	return nil
}

func fromRecord(bytes []byte) (*todo, bool) {
	var record struct {
		ID      *int64  `json:"id"`
		Title   *string `json:"title"`
		Done    *bool   `json:"done"`
		Version *int64  `json:"version"`
		Deleted *bool   `json:"deleted"`
	}
	if json.Unmarshal(bytes, &record) != nil || record.ID == nil || record.Title == nil ||
		record.Done == nil || record.Version == nil || record.Deleted == nil {
		return nil, false
	}
	return &todo{*record.ID, *record.Title, *record.Done, *record.Version, *record.Deleted}, true
}

func load(data string) (map[int64]*todo, error) {
	if err := os.MkdirAll(filepath.Join(data, "todos"), 0o755); err != nil {
		return nil, err
	}
	todos := map[int64]*todo{}
	shards, err := os.ReadDir(data)
	if err != nil {
		return nil, err
	}
	for _, shard := range shards {
		if !shard.IsDir() || !isShard(shard.Name()) {
			continue
		}
		entries, err := os.ReadDir(filepath.Join(data, shard.Name()))
		if err != nil {
			return nil, err
		}
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			dir := filepath.Join(data, shard.Name(), entry.Name())
			files, err := os.ReadDir(dir)
			if err != nil {
				return nil, err
			}
			var found *todo
			for _, file := range files {
				if _, ok := versionOf(file.Name()); !ok {
					continue
				}
				bytes, err := os.ReadFile(filepath.Join(dir, file.Name()))
				if err != nil {
					continue
				}
				if t, ok := fromRecord(bytes); ok && (found == nil || found.version < t.version) {
					found = t
				}
			}
			if found != nil {
				todos[found.id] = found
			}
		}
	}
	return todos, nil
}

var reasons = map[int]string{
	400: "Bad Request",
	404: "Not Found",
	405: "Method Not Allowed",
	413: "Content Too Large",
}

func respond(w http.ResponseWriter, status int, contentType, body string) {
	if contentType != "" {
		w.Header().Set("Content-Type", contentType)
	}
	w.Header().Set("Content-Length", strconv.Itoa(len(body)))
	w.WriteHeader(status)
	io.WriteString(w, body)
}

func plain(w http.ResponseWriter, status int, body string) {
	respond(w, status, "text/plain; charset=utf-8", body)
}

func statusResponse(w http.ResponseWriter, status int) {
	if status == 204 {
		w.WriteHeader(204)
		return
	}
	plain(w, status, reasons[status]+"\n")
}

func methodNotAllowed(w http.ResponseWriter, allow string) {
	w.Header().Set("Allow", allow)
	statusResponse(w, 405)
}

func (s *state) stored(w http.ResponseWriter, t todo, ok func()) {
	if s.save(t) != nil {
		plain(w, 500, "not stored\n")
		return
	}
	ok()
}

func parseBody(body []byte) (any, bool) {
	var value any
	if json.Unmarshal(body, &value) != nil {
		return nil, false
	}
	return value, true
}

func member[T any](value any, name string) (T, bool) {
	var zero T
	object, ok := value.(map[string]any)
	if !ok {
		return zero, false
	}
	v, ok := object[name].(T)
	return v, ok
}

func (s *state) list(w http.ResponseWriter) {
	s.mu.Lock()
	ids := make([]int64, 0, len(s.todos))
	for id, t := range s.todos {
		if !t.deleted {
			ids = append(ids, id)
		}
	}
	sort.Slice(ids, func(a, b int) bool { return ids[a] < ids[b] })
	var out strings.Builder
	out.WriteByte('[')
	for i, id := range ids {
		if i > 0 {
			out.WriteByte(',')
		}
		toJSON(s.todos[id], &out)
	}
	out.WriteByte(']')
	s.mu.Unlock()
	respond(w, 200, "application/json", out.String())
}

func (s *state) create(w http.ResponseWriter, body []byte) {
	value, ok := parseBody(body)
	if !ok {
		statusResponse(w, 400)
		return
	}
	title, ok := member[string](value, "title")
	if !ok {
		statusResponse(w, 400)
		return
	}
	done, _ := member[bool](value, "done")
	s.mu.Lock()
	id := s.nextID + 1
	for s.todos[id] != nil {
		id++
	}
	s.nextID = id
	t := &todo{id: id, title: title, done: done, version: 1}
	s.todos[id] = t
	copied := *t
	s.mu.Unlock()
	s.stored(w, copied, func() { s.answer(w, 201, &copied) })
}

func (s *state) answer(w http.ResponseWriter, status int, t *todo) {
	var out strings.Builder
	toJSON(t, &out)
	respond(w, status, "application/json", out.String())
}

func (s *state) live(id int64) (todo, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	t := s.todos[id]
	if t == nil || t.deleted {
		return todo{}, false
	}
	return *t, true
}

func (s *state) show(w http.ResponseWriter, id int64) {
	t, ok := s.live(id)
	if !ok {
		statusResponse(w, 404)
		return
	}
	s.answer(w, 200, &t)
}

func (s *state) patch(w http.ResponseWriter, id int64, body []byte) {
	value, ok := parseBody(body)
	if !ok {
		if _, found := s.live(id); found {
			statusResponse(w, 400)
		} else {
			statusResponse(w, 404)
		}
		return
	}
	s.mu.Lock()
	t := s.todos[id]
	if t == nil || t.deleted {
		s.mu.Unlock()
		statusResponse(w, 404)
		return
	}
	if title, ok := member[string](value, "title"); ok {
		t.title = title
	}
	if done, ok := member[bool](value, "done"); ok {
		t.done = done
	}
	t.version++
	copied := *t
	s.mu.Unlock()
	s.stored(w, copied, func() { s.answer(w, 200, &copied) })
}

func (s *state) delete(w http.ResponseWriter, id int64) {
	s.mu.Lock()
	t := s.todos[id]
	if t == nil || t.deleted {
		s.mu.Unlock()
		statusResponse(w, 404)
		return
	}
	t.deleted = true
	t.version++
	copied := *t
	s.mu.Unlock()
	s.stored(w, copied, func() { statusResponse(w, 204) })
}

func (s *state) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	body, err := io.ReadAll(io.LimitReader(r.Body, bodyLimit+1))
	if err != nil || len(body) > bodyLimit {
		statusResponse(w, 413)
		return
	}
	var segments []string
	for _, segment := range strings.Split(r.URL.EscapedPath(), "/") {
		if segment != "" {
			segments = append(segments, segment)
		}
	}
	switch {
	case r.Method == "GET" && len(segments) == 1 && segments[0] == "health":
		plain(w, 200, "ok\n")
	case len(segments) == 1 && segments[0] == "todos":
		switch r.Method {
		case "GET":
			s.list(w)
		case "POST":
			s.create(w, body)
		default:
			methodNotAllowed(w, "GET, POST")
		}
	case len(segments) == 2 && segments[0] == "todos":
		id, ok := parseInt(segments[1])
		if !ok {
			statusResponse(w, 404)
			return
		}
		switch r.Method {
		case "GET":
			s.show(w, id)
		case "PATCH":
			s.patch(w, id, body)
		case "DELETE":
			s.delete(w, id)
		default:
			methodNotAllowed(w, "GET, PATCH, DELETE")
		}
	default:
		statusResponse(w, 404)
	}
}

func main() {
	port := flag.Int("port", 8080, "port on 127.0.0.1")
	flag.Parse()
	data := "todo-data"
	todos, err := load(data)
	if err != nil {
		fmt.Fprintf(os.Stderr, "todo: loading %s: %v\n", data, err)
		os.Exit(2)
	}
	listener, err := net.Listen("tcp", fmt.Sprintf("127.0.0.1:%d", *port))
	if err != nil {
		fmt.Fprintf(os.Stderr, "todo: %v\n", err)
		os.Exit(2)
	}
	fmt.Fprintf(os.Stderr, "listening on %s\n", listener.Addr())
	server := &http.Server{Handler: &state{todos: todos, data: data}}
	server.Serve(listener)
}
