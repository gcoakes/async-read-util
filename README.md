# async-read-util

A collection of utilities for working with AsyncRead. I wrote this originally to
support hashing and extracting a gzipped file in the form of an
`futures::AsyncRead`. `ObservedReader` is used to hash without mutating the
data, and `MappedReader` is used to decompress the data.
