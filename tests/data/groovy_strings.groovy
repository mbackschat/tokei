#!/usr/bin/env groovy
// 22 lines 17 code 4 comments 1 blanks
def url = 'http://example.org' // a comment after a single-quoted string
def text = '''first

last'''
def name = "it's ${url.replace('/', "//")}"
def doc = """a ${ text /* a comment in a hole */ } b

c"""
def path = /C:\dir\// // a slashy string ending in an escaped slash
def ratio = 4 / 2 // division
def dollar = $/a /* no comment */ b/$
/* a block comment
   on two lines */

def s = 'quote " inside'
println(url + text + name + doc + path + ratio + dollar + s)
def multi = /line one
line two/
def q = "x" /* after */
assert true
