# NOTES

## BASIC FACE DETECTION CHALLENGE

* install opencv dependencies:
<https://github.com/twistedfall/opencv-rust/blob/master/INSTALL.md>
* download haarcascade_frontalface_alt2.xml from <https://github.com/opencv/opencv/blob/master/data/haarcascades/haarcascade_frontalface_alt2.xml>
* put in data folder

## VISUAL BASIC MATH CHALLENGE

* Install PaddleOCR engine -> <https://github.com/PaddlePaddle/PaddleOCR>

```bash
pip install paddlepaddle paddleocr
```

* Example command

```bash
paddleocr ocr -i ./data/math_2.jpeg --use_doc_orientation_classify False --use_doc_unwarping False --use_textline_orientation False --rec_char_dict_path --save_path ./output
```
