;; (def-msg language-preference :lang 'string)
(defstruct language-preference
  (lang "" :type string))

;; (def-msg author-list :names (list 'string))
(defstruct author-list
  (names "" :type (cons string)))
