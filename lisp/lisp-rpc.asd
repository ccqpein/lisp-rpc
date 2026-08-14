;;;; -*- Mode: Lisp; Syntax: ANSI-Common-Lisp; Base: 10 -*-

(defpackage :lisp-rpc-sys
  (:documentation "define the lisp-rpc system")
  (:use :CL :asdf))

(in-package :lisp-rpc-sys)

(defsystem :lisp-rpc
  :description "Lisp RPC"
  :version "0.0.1"
  :depends-on ("str" "alexandria" "woo" "bordeaux-threads" "flexi-streams" "clingon")
  :components ((:file "util")
               (:file "raw-data")
               (:file "generator")
               (:file "rpc-server")
               (:file "cli"))
  :build-operation "program-op"
  :build-pathname "lisp-rpc-gen"
  :entry-point "lisp-rpc-cli:main")

#+sb-core-compression
(defmethod asdf:perform ((o asdf:image-op) (c asdf:system))
  (uiop:dump-image (asdf:output-file o c)
                   :executable t
                   :compression t))
