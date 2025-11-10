#!/bin/sh
mkdir converted
magick mogrify -path converted -define png:color-type=6 -format png *.png
