(defpackage raw-data
  (:use #:cl))

(in-package :raw-data)

(defun run ()
  (let ((data '("(hello-world)"
                "(hello-world :from \"USA\")"
                "(hello-world :from \"Japan\" 
:my-name \"Mikasa\")"
                "(hello-world :from '(\"Japan\" \"Tokyo\") 
:my-name \"Mikasa\")"))
        (data1 "(nest-hello :from (hello-world :from \"USA\"))"))

    (loop for d in data
          do (let ((rd (lisp-rpc-raw-data:parse-data d)))
               (format t "name: ~a, from: ~a, name: ~a~%"
                       (lisp-rpc-raw-data:get-name rd)
                       (lisp-rpc-raw-data:data-get rd :from)
                       (lisp-rpc-raw-data:data-get rd :name))))

    (let* ((rd (lisp-rpc-raw-data:parse-data data1))
           (nest-rd (lisp-rpc-raw-data:data-get rd :from)))
      (format t "name: ~a~%" (lisp-rpc-raw-data:get-name rd))
      (format t "in nest data's from, name: ~a, from: ~a"
              (lisp-rpc-raw-data:get-name nest-rd)
              (lisp-rpc-raw-data:data-get nest-rd :from)))
    ))
