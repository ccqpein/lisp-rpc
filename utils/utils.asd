;;;; -*- Mode: Lisp -*-

(defpackage :utils-sys
  (:use :CL :asdf))

(in-package :utils-sys)

(defsystem :utils
  :description "Utility package for lisp-rpc spec checking"
  :version "0.0.1"
  :depends-on ("str" "alexandria")
  :components ((:file "spec-checker")))

(defsystem :utils/tests
  :description "Test suite for utils"
  :depends-on ("utils" "fiveam")
  :components ((:file "spec-checker-test"))
  :perform (test-op (o s)
                    (uiop:symbol-call :fiveam :run!
                                      (uiop:find-symbol* :test-lisp-rpc :test-lisp-rpc))))

#+sb-core-compression
(defmethod asdf:perform ((o asdf:image-op) (c asdf:system))
  (uiop:dump-image (asdf:output-file o c)
                   :executable t
                   :compression t))
