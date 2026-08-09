(defpackage lisp-rpc-generator
  (:use #:cl))

(in-package :lisp-rpc-generator)

(defun def-funs-router (name key args)
  "pick which function should I call.
Return the function and the name of the type"
  (cond
    ((and (typep args 'cons) (keywordp (first args)))
     ;; the function and the new map struct name
     (list #'def-map (read-from-string (format nil "~a-~a" name key))))
    
    ((and (typep args 'cons) (eq (first args) 'list))
     ;; it has to be the (list 'string), cannot missing the string's quote
     (list #'def-list `(cons ,(eval (second args)))))

    ((and (typep args 'cons) (eq (first args) 'optional))
     (list #'def-optional (eval (second args))))
    
    (t (error "dont know which function should use for ~a" args))))

(defun def-msg (msg-expression &optional stream)
  (unless (eq 'def-msg (first msg-expression))
    (error "fisrt elemenet of msg-expression has to be the def-msg"))
  (format stream "~a~%~%" (prin1-to-string (def-msg-struct (second msg-expression) (cddr msg-expression) stream)))
  (format stream "~a~%~%" (prin1-to-string (def-msg-to-lisp-rpc-data
                                               (second msg-expression)
                                               (loop for (k a) on (cddr msg-expression) by #'cddr
                                                     collect k)))))

(defun def-msg-struct (name args &optional stream)
  `(defstruct ,name
     ,@(loop for (k tt) on args by #'cddr
             collect (let ((tt (if (and (consp tt) (eq (first tt) 'quote))
                                   (eval tt)
                                   tt)))
                       (cons (intern (string k))
                             (cond ((equal 'string tt)
                                    (list "" :type 'string))
                                   ((equal 'number tt)
                                    (list 0 :type 'number))
                                   ;; anonymous type
                                   ((typep tt 'cons)
                                    (let* ((router-result (def-funs-router name k tt)))
                                      (format t "router-result: ~a~%" router-result)
                                      (funcall (first router-result) (second router-result) tt stream)
                                      (list nil :type (second router-result))))
                                   ;; default
                                   (t (list nil :type tt))))))))

(defun def-msg-to-lisp-rpc-data (name keys &optional stream)
  (declare (ignore stream))
  `(defmethod to-lisp-rpc-data ((x ,name) &rest args &key &allow-other-keys)
     (declare (ignore args))
     (format nil
             ,(format nil "(~a ~~{:~~1{~~A ~~A~~}~~^ ~~})" name)
             (list ,@(loop for k in keys
                           collect `(list (quote ,(intern (symbol-name k)))
                                          (to-lisp-rpc-data (,(read-from-string (format nil "~a-~a" name k)) x))))))))

(def-msg (read-from-string "(def-msg language-preference :lang 'string)") t)

(def-msg (read-from-string "(def-msg book-info
  :lang 'language-preference
  :title 'string
  :version 'string
  :id 'string)") t)

(def-msg (read-from-string "(def-msg book-info
  :lang '(:lang 'string :encoding 'number)
  :title 'string)") t)

(def-msg (read-from-string "(def-msg authors :names (list 'string))") t)

(defun def-map (name map-kv-pairs &optional stream)
  "kind of the def-msg, but the defmethod define isn't same"
  (format stream "~a~%~%" (prin1-to-string (def-msg-struct name map-kv-pairs stream)))
  (format stream "~a~%~%" (prin1-to-string (def-map-to-lisp-rpc-data
                                               name
                                               (loop for (k a) on map-kv-pairs by #'cddr
                                                     collect k)))))

(defun def-map-to-lisp-rpc-data (name keys &optional stream)
  (declare (ignore stream))
  `(defmethod to-lisp-rpc-data ((x ,name) &rest args &key &allow-other-keys)
     (declare (ignore args))
     (format nil
             "(~{:~1{~A ~A~}~^ ~})"
             (list ,@(loop for k in keys
                           collect `(list (quote ,(intern (symbol-name k)))
                                          (to-lisp-rpc-data (,(read-from-string (format nil "~a-~a" name k)) x))))))))

(defun def-list (name args &optional stream)
  "(list 'string), placeholder, no need to make the additional struct"
  (declare (ignore name args stream)))

(defun def-optional (name args &optional stream)
  "(optional 'string), placeholder, look like all fields in lisp struct is optional anyway"
  (declare (ignore name args stream)))

(defun def-rpc (rpc-expression &optional stream)
  (unless (eq 'def-rpc (first rpc-expression))
    (error "fisrt elemenet of rpc-expression has to be the def-rpc"))
  ;; generated struct like msg
  (format stream "~a~%~%" (prin1-to-string (def-msg-struct (second rpc-expression) (cddr rpc-expression) stream)))
  (format stream "~a~%~%" (prin1-to-string (def-msg-to-lisp-rpc-data
                                               (second msg-expression)
                                               (loop for (k a) on (cddr msg-expression) by #'cddr
                                                     collect k)))))
