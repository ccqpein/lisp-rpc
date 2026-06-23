;;;; -*- Mode: Lisp; Syntax: ANSI-Common-Lisp; Base: 10 -*-

(defpackage :lisp-rpc-sys
  (:documentation "define the lisp-rpc system")
  (:use :CL :asdf))

(in-package :lisp-rpc-sys)

(defsystem :lisp-rpc
  :description "Lisp RPC"
  :version "0.0.1"
  :depends-on ("str" "alexandria")
  :components ((:file "raw-data"))
  :in-order-to ((test-op (test-op "lisp-rpc/tests"))))

(defsystem :lisp-rpc/tests
  :description "Test suite for lisp-rpc"
  :depends-on ("lisp-rpc" "fiveam")
  :components ((:file "raw-data-test"))
  :perform (test-op (o s)
                    (uiop:symbol-call :fiveam :run!
                                      (uiop:find-symbol* :test-raw-data :test-lisp-rpc-raw-data))))
