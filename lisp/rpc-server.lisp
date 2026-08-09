(defpackage lisp-rpc-server
  (:use #:cl #:common)
  (:export #:rpc-server
           #:make-rpc-server
           #:rpc-server-p
           #:rpc-server-host
           #:rpc-server-port
           #:rpc-server-handlers
           #:rpc-server-handler-object
           #:register-rpc-handler
           #:start-server
           #:stop-server
           #:handle-rpc))

(in-package :lisp-rpc-server)

(defgeneric handle-rpc (handler req)
  (:documentation "Generic function for CLOS method dispatch handlers.")
  (:method (handler req)
    (error "No handle-rpc method implemented for handler ~S and request ~S" handler req)))

(defstruct rpc-server
  "RPC server wrapping Woo HTTP server."
  (host "127.0.0.1" :type string)
  (port 5000 :type integer)
  (handlers (make-hash-table :test 'eq))
  handler-object
  worker-thread)

(defun register-rpc-handler (server rpc-name handler-fn)
  "Register a handler function for a specific def-rpc endpoint symbol."
  (let* ((cls (find-class rpc-name nil))
         (dummy-instance (when cls (ignore-errors (allocate-instance cls)))))
    (unless (and dummy-instance (rpc-endpoint-p dummy-instance))
      (error "Registration Error: ~S is not a valid def-rpc endpoint." rpc-name)))
  (setf (gethash rpc-name (rpc-server-handlers server)) handler-fn)
  (format t "Registered RPC handler for: ~A~%" rpc-name)
  rpc-name)

(defun make-woo-app (server)
  (lambda (env)
    (let ((method (getf env :request-method))
          (raw-body (getf env :raw-body)))
      (if (not (eq method :POST))
          '(405 (:content-type "text/plain") ("Method Not Allowed"))
          (handler-case
              (let* ((body-bytes (alexandria:read-stream-content-into-byte-vector raw-body))
                     (body-str (flexi-streams:octets-to-string body-bytes :external-format :utf-8))
                     (req-obj (from-lisp-rpc-data body-str)))
                
                ;; 1. Check if request object is a registered def-rpc endpoint
                (unless (rpc-endpoint-p req-obj)
                  (error "Invalid RPC Request: ~S is not a registered def-rpc endpoint." (type-of req-obj)))
                
                (let* ((rpc-name (type-of req-obj))
                       (fn-handler (gethash rpc-name (rpc-server-handlers server)))
                       (expected-resp-type (rpc-response-type req-obj))
                       (resp-obj (cond
                                   (fn-handler (funcall fn-handler req-obj))
                                   ((rpc-server-handler-object server)
                                    (handle-rpc (rpc-server-handler-object server) req-obj))
                                   (t (error "No handler registered for RPC ~A" rpc-name)))))
                  
                  ;; 2. Enforce response type matching
                  (when (and expected-resp-type (not (typep resp-obj expected-resp-type)))
                    (error "RPC ~A handler returned invalid response type ~S. Expected type ~A."
                           rpc-name (type-of resp-obj) expected-resp-type))
                  
                  `(200 (:content-type "text/plain") (,(to-lisp-rpc-data resp-obj)))))
            (error (c)
              `(400 (:content-type "text/plain") (,(format nil "RPC Error: ~A" c)))))))))

(defun start-server (server &key (async t))
  "Start Woo server for rpc-server instance."
  (let ((app (make-woo-app server)))
    (if async
        (setf (rpc-server-worker-thread server)
              (bt:make-thread (lambda ()
                                (woo:run app
                                         :host (rpc-server-host server)
                                         :port (rpc-server-port server)))))
        (woo:run app
                 :host (rpc-server-host server)
                 :port (rpc-server-port server)))))

(defun stop-server (server)
  "Stop running worker thread of rpc-server instance."
  (when (rpc-server-worker-thread server)
    (bt:destroy-thread (rpc-server-worker-thread server))
    (setf (rpc-server-worker-thread server) nil)))
