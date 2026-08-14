(defpackage :lisp-rpc-cli
  (:use #:cl)
  (:export #:main))

(in-package :lisp-rpc-cli)

(defun gen-cmd-options ()
  (list
   (clingon:make-option
    :string
    :description "Path to specification file (e.g., spec.lisp)"
    :short-name #\s
    :long-name "spec"
    :key :spec)
   (clingon:make-option
    :string
    :description "Output directory for the generated project"
    :short-name #\o
    :long-name "output"
    :initial-value "./output/"
    :key :output)))

(defun gen-cmd-handler (cmd)
  (let ((spec-file (or (clingon:getopt cmd :spec)
                       (first (clingon:command-arguments cmd))))
        (output-dir (clingon:getopt cmd :output)))
    (if (or (null spec-file) (string= spec-file ""))
        (progn
          (format *error-output* "Error: Missing required specification file argument <SPEC-FILE>.~%~%")
          (clingon:print-usage-and-exit cmd t))
        (handler-case
            (multiple-value-bind (asd-path lib-path)
                (lisp-rpc-generator:generate-project spec-file output-dir)
              (format t "Successfully generated Lisp RPC project!~%")
              (format t "  ASD file: ~A~%" asd-path)
              (format t "  Lib file: ~A~%" lib-path))
          (error (c)
            (format *error-output* "Error generating project: ~A~%" c)
            (uiop:quit 1))))))

(defun lisp-rpc-cli-command ()
  (clingon:make-command
   :name "lisp-rpc-gen"
   :description "Code generator CLI for Lisp-RPC specifications"
   :version "0.0.1"
   :handler (lambda (cmd) (clingon:print-usage-and-exit cmd t))
   :sub-commands (list
                  (clingon:make-command
                   :name "gen"
                   :aliases '("generate")
                   :description "Generate a Lisp project from a spec file"
                   :options (gen-cmd-options)
                   :handler #'gen-cmd-handler))))

(defun main ()
  (clingon:run (lisp-rpc-cli-command)))
