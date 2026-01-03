(ql:quickload '("str" "alexandria"))

(defpackage lisp-rpc-checker
  (:use #:cl))

(in-package :lisp-rpc-checker)

(defparameter *example* "(def-msg language-perfer :lang 'string)

  (def-msg book-info
    :lang 'language-perfer
    :title 'string
    :version 'string
    :id 'string)

  (def-rpc get-book
      '(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))
    'book-info)

  (def-msg language-perfers :langs '(list 'string))")

(defparameter *checker-map*
  (mapcar #'cons
       '("def-msg" "def-rpc" "def-rpc-package")
       '(def-msg-checker def-rpc-checker def-rpc-package-check)))

(defun spec-check-file (spec-file)
  (with-open-file (stream spec-file :direction :input)
    (loop for expr = (read stream nil :eof)
          ;;for count from 0
          until (eq expr :eof)
          ;;do (format t "expr: ~a check result: ~a~%" expr (spec-check-one expr))
          unless (handler-case (spec-check-one expr)
                   (error (e)
                     (format t "expr: ~a check failed with error" expr)
                     (return-from spec-check-file e)))
            do (format t "expr: ~a check failed" expr)
            and return nil
          finally (return t))))

(defun spec-check-one (expr)
  "get one expr, check it roughly and try to eval real checker it"
  (if (< (length expr) 2)
      (error "spec expr at least have two elements inside"))
  (let ((x (first expr))        
        checker)
    (loop for (sx . c) in *checker-map*
          when (or (string= x sx) (string= x (str:upcase sx)))
            do (setf checker c)
            and return nil
          finally
             (error (format nil
                            "spec expr only support the ~{~a~^, ~}"
                            (mapcar #'car *checker-map*))))
    (funcall checker (second expr) (cddr expr))))

(defun def-msg-checker (name args)
  (declare (ignore name))
  (if (zerop (length args))
      ;; it can be the empty definations
      t
      ;; the rest should be some map data format
      (map-data-type-checker args)))

(defun type-checker (ty)
  "check the type defination"
  (ctypecase ty
    (keyword nil)
    (symbol (unless (not ty) 1)) ;; symbol is 0
    (cons (if (equal 'quote (first ty))
              (type-checker (second ty))
              (cond ((map-data-type-checker ty) 1) ;; map is 1
                    ((apply #'list-type-checker ty) 2) ;; list is 2
                    )))))

(defun map-data-type-checker (eles)
  "check map data format. 
can be used to check the data and the type defination"
  (if (zerop (length eles)) (return-from map-data-type-checker nil))
  (loop for (k v) on eles by #'cddr
        unless (and (keywordp k)
                    (type-checker v))
          return nil
        finally (return t)))

(defun list-type-checker (&rest args)
  "check list type *defination*. 
list type defination should be '(list 'other-type)"
  (if (/= (length args) 2) (return-from list-type-checker nil))
  (and (eq (first args) 'list)
       (type-checker (second args))))

(defun list-data-type-checker (eles)
  "this one check the list data. list type defination should use the list-type-checker"
  (every (lambda (e) (eq (type-of (first eles))
                         (type-of e)))
         eles))

(defun def-rpc-checker (name args)
  "args has to be the ('(map-data) symbol), the first args will be eval"
  (declare (ignore name))
  (if (zerop (length args))
      ;; it can be the empty definations
      (return-from def-rpc-checker t))
  (and (= 1 (funcall #'type-checker (first args)))
       (if (second args) (type-checker (second args)) t)))

(defun def-rpc-package-check (name &rest args)
  t)
