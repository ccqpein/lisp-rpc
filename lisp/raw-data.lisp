;;; implenment the raw data
(defpackage lisp-rpc-raw-data
  (:use #:cl)
  (:export #:raw-data
           #:make-raw-data
           #:raw-data-p
           #:raw-data-name
           #:raw-data-kv
           #:get-name
           #:data-get

           #:raw-data-map
           #:make-raw-data-map
           #:raw-data-map-p
           #:raw-data-map-kv
           #:map-data-get

           #:raw-data-list
           #:make-raw-data-list
           #:raw-data-list-p
           #:raw-data-list-l

           #:type-of-raw-data

           #:parse-data
           #:parse-raw-data
           #:parse-raw-map-data
           #:parse-raw-list-data))

(in-package :lisp-rpc-raw-data)

(defstruct raw-data
  "Raw data structure"

  ;; the name of raw data
  name

  ;; key-values pairs
  kv)

(defmethod get-name ((d raw-data) &key &allow-other-keys)
  (raw-data-name d))

(defmethod data-get ((d raw-data) key &key &allow-other-keys)
  (map-data-get (raw-data-kv d) key))

(defstruct raw-data-map
  "Raw map"
  kv)

(defmethod map-data-get ((d raw-data-map) key &key &allow-other-keys)
  (getf (raw-data-map-kv d) key))

(defstruct raw-data-list
  "Raw list"
  l)

(defun type-of-raw-data (raw-data)
  (cond ((consp raw-data)
         ;; list
         (cond ((eq 'quote (first raw-data))
                (type-of-raw-data (eval raw-data)))
               ((keywordp (first raw-data))
                ;; map actually is the plist
                ;; keyword is symbol, so need the symbol first
                :map)
               ((and (symbolp (first raw-data))
                     (keywordp (second raw-data)))
                :data)
               (t :list)))
        ((stringp raw-data)
         :string)
        ((numberp raw-data)
         :number)))

(defun parse-data (data)
  "entry of the raw string"
  (let ((raw-data (read (make-string-input-stream data))))
    (parse-raw-data raw-data)))

(defun parse-raw-data (raw-data)
  "parse the lisp-rpc data"
  (case (type-of-raw-data raw-data)
    ((:number :string) raw-data)
    (:map (parse-raw-map-data raw-data))
    (:list (parse-raw-list-data raw-data))
    (t
     ;; data below
     (do ((this-ty 'symbol)
          (kv-cache)
          (rest-data raw-data (cdr rest-data))
          (rd (make-raw-data)))
         ((null rest-data)
          (setf (raw-data-kv rd) (make-raw-data-map :kv kv-cache))
          rd)
       (case this-ty
         (symbol (setf (raw-data-name rd) (first rest-data)
                       this-ty 'key))
         (key (setf kv-cache (append kv-cache (list (first rest-data)))
                    this-ty 'value))
         (value (setf kv-cache (append kv-cache (list (parse-raw-data (first rest-data))))
                      this-ty 'key)))))))

(defun parse-raw-map-data (raw-map-data)
  "parse the map data: (:a 1 :b 2)"
  (let ((mm (if (eq 'quote (first raw-map-data))
                (eval raw-map-data) ;; unquoted since in lisp-rpc, the map has to been quote
                raw-map-data)))
    (make-raw-data-map :kv mm)))

(defun parse-raw-list-data (raw-list-data)
  (let ((mm (if (eq 'quote (first raw-list-data))
                (eval raw-list-data) ;; unquoted
                raw-list-data)))
    (make-raw-data-list :l mm)))

