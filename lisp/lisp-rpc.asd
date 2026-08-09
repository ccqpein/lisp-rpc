;;;; -*- Mode: Lisp; Syntax: ANSI-Common-Lisp; Base: 10 -*-

(defpackage :lisp-rpc-sys
  (:documentation "define the lisp-rpc system")
  (:use :CL :asdf))

(in-package :lisp-rpc-sys)

(defsystem :lisp-rpc
  :description "Lisp RPC"
  :version "0.0.1"
  :depends-on ("str" "alexandria")
  :components ((:file "common")
               (:file "raw-data"))
  )
