#import "../lib/zettel.typ": zettel
#show: zettel

#let abstract(body) = block[
  #emph[Abstract.] #body
]
#let keywords(..items) = block[
  #emph[Keywords.] #items.pos().join[, ]
]
#let category = (
  thoughts: [Thoughts],
  physics: [Physics],
  coding: [Coding],
)

= Untitled <new>

// @zk-field "abstract" kind=markup
#abstract[]

// @zk-field "keywords" kind=string-list
#keywords()

// @zk-field "category" kind=string
#category.thoughts
