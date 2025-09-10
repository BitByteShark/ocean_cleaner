#! /bin/bash

infile_basename=$(basename "$1")
if [[ ! "$infile_basename" =~ \.epub$ ]]
then
    echo "only .epub input files"
    exit 1
fi

# prepare temp dir
tmp_extraction_target_dir="$(mktemp -d)"

# unzip
unzip -q "$1" -d "$tmp_extraction_target_dir"

# remove marks
rm "$tmp_extraction_target_dir/oceanofpdf.com"
replacement_term='s/<div .*OceanofPDF.*<\/div>//'
find "$tmp_extraction_target_dir" -type f -print0 | xargs -0 sed -i "${replacement_term}" #--debug | grep "MATCHED" -B2 -A2

# rezip
output_file="$(pwd)/"
if [[ $infile_basename == _OceanofPDF.com_* ]]
then
    output_file+="${infile_basename:16}"
else

    output_file+="${infile_basename%.epub}_no_watermarks.epub"
fi

(cd "$tmp_extraction_target_dir" && zip -qrX0 "$output_file" .)

# cleanup temp dir
rm -r "$tmp_extraction_target_dir"
