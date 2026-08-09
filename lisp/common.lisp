(defpackage common
  (:use #:cl)
  (:export #:to-lisp-rpc-data
           #:from-lisp-rpc-data
           #:rpc-endpoint-p
           #:rpc-response-type))

(in-package :common)

(defgeneric rpc-endpoint-p (req)
  (:documentation "Returns T if REQ is a top-level callable RPC endpoint struct.")
  (:method (req) nil))

(defgeneric rpc-response-type (req)
  (:documentation "Returns the expected response type symbol for an RPC request struct.")
  (:method (req) nil))

(defmethod to-lisp-rpc-data ((x string) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "~S" x))

(defmethod to-lisp-rpc-data ((x number) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "~a" x))

(defmethod to-lisp-rpc-data ((x cons) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "'(~{~a~^ ~})" (mapcar #'to-lisp-rpc-data x)))

(defun from-lisp-rpc-data (data &rest args &key &allow-other-keys)
  (declare (ignore args))
  (cond
    ((stringp data)
     (let ((parsed (read-from-string data)))
       (cond
         ((or (stringp parsed) (numberp parsed) (consp parsed))
          (from-lisp-rpc-data parsed))
         (t data))))
    ((consp data)
     (cond
       ;; Unquote '(...)
       ((eq (first data) 'quote)
        (from-lisp-rpc-data (second data)))

       ;; Struct pattern: (struct-name :key1 val1 :key2 val2 ...)
       ((and (symbolp (first data))
             (keywordp (second data))
             (let ((ctor (find-symbol (format nil "MAKE-~A" (symbol-name (first data)))
                                      (symbol-package (first data)))))
               (and ctor (fboundp ctor))))
        (let ((ctor (find-symbol (format nil "MAKE-~A" (symbol-name (first data)))
                                 (symbol-package (first data)))))
          (apply ctor
                 (loop for (k v) on (cdr data) by #'cddr
                       append (list k (from-lisp-rpc-data v))))))

       ;; Map / Plist pattern: (:key1 val1 :key2 val2)
       ((keywordp (first data))
        (loop for (k v) on data by #'cddr
              append (list k (from-lisp-rpc-data v))))

       ;; Standard list: (elem1 elem2 ...)
       (t
        (mapcar #'from-lisp-rpc-data data))))
    (t data)))
