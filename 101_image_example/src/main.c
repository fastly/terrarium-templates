#include "http_hostcalls.h"

#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "image.h"
#include "jpeg/jpeglib.h"

// Resize an image
static void resize(Image *out, const Image *in)
{
    unsigned int  x, y, d;
    unsigned long sx, sy;
    unsigned long stepx = in->width * THETA / out->width, stepy = in->height * THETA / out->height;
    unsigned int  isx, isy, rsx, rsy;

    sy = 0;
    for (y = 0; y < out->height - 1; y++) {
        sx  = 0;
        isy = sy / THETA, rsy = sy % THETA;
        for (x = 0; x < out->width - 1; x++) {
            isx = sx / THETA, rsx = sx % THETA;
            for (d = 0; d < in->depth; d++) {
                const unsigned char *p = &in->buf[isy * in->row_stride + isx * in->depth + d];
                const unsigned int   z = p[0], zr = p[in->depth], zd = p[in->row_stride],
                                   zdr = p[in->row_stride + in->depth];
                out->buf[y * out->row_stride + x * in->depth + d] =
                    (z * (THETA - rsx) + zr * rsx + z * (THETA - rsy) + zd * rsy +
                     z * ((2 * THETA) - (rsx + rsy)) + zdr * (rsx + rsy)) /
                    (4 * THETA);
            }
            sx += stepx;
        }
        isx = sx / THETA, rsx = sx % THETA;
        for (d = 0; d < in->depth; d++) {
            const unsigned char *p = &in->buf[isy * in->row_stride + isx * in->depth + d];
            const unsigned int   z = p[0], zr = z, zd = p[in->row_stride], zdr = zd;
            out->buf[y * out->row_stride + x * in->depth + d] =
                (z * (THETA - rsx) + zr * rsx + z * (THETA - rsy) + zd * rsy +
                 z * ((2 * THETA) - (rsx + rsy)) + zdr * (rsx + rsy)) /
                (4 * THETA);
        }
        sy += stepy;
    }
    for (x = 0; x < out->width - 1; x++) {
        isx = sx / THETA, rsx = sx % THETA;
        for (d = 0; d < in->depth; d++) {
            const unsigned char *p = &in->buf[isy * in->row_stride + isx * in->depth + d];
            const unsigned int   z = p[0], zr = p[in->depth], zd = z, zdr = zr;
            out->buf[y * out->row_stride + x * in->depth + d] =
                (z * (THETA - rsx) + zr * rsx + z * (THETA - rsy) + zd * rsy +
                 z * ((2 * THETA) - (rsx + rsy)) + zdr * (rsx + rsy)) /
                (4 * THETA);
        }
        sx += stepx;
    }
    isx = sx / THETA, rsx = sx % THETA;
    for (d = 0; d < in->depth; d++) {
        const unsigned char *p = &in->buf[isy * in->row_stride + isx * in->depth + d];
        out->buf[y * out->row_stride + x * in->depth + d] = p[0];
    }
}

// Apply a sharpening filter
static void convolution(Image *out, const Image *in)
{
    unsigned int x, y, d;

    for (y = 1; y < in->height - 1; y++) {
        for (x = 1; x < in->width - 1; x++) {
            for (d = 0; d < in->depth; d++) {
                const unsigned char *p = &in->buf[y * in->row_stride + x * in->depth + d];
                long z = 5 * p[0] - p[-(signed long) in->row_stride] - p[in->row_stride] -
                         p[-(signed int) in->depth] - p[in->depth];
                if (z < 0) {
                    z = 0;
                } else if (z > 255) {
                    z = 255;
                }
                out->buf[y * in->row_stride + x * in->depth + d] = z;
            }
        }
    }
}

// Merge two layers with a given opacity (0=transparent, 100=opaque)
static void layer_merge(Image *out, const Image *in, unsigned int opacity)
{
    unsigned long i;
    unsigned long w1 = opacity, w2 = OPACITY_MAX - w1, wt = OPACITY_MAX;

    for (i = 0; i < in->buf_len; i++) {
        out->buf[i] = (w1 * out->buf[i] + w2 * in->buf[i]) / wt;
    }
}

// Exit with an error message
static int error(const char *msg)
{
    hostcall_resp_set_response_code(RESPONSE_OUTGOING, 500);
    set_nocache(RESPONSE_OUTGOING);
    hostcall_resp_set_body(RESPONSE_OUTGOING, msg, strlen(msg));
    return -1;
}

// Allocate a new image structure
static int image_new(Image *image, unsigned int width, unsigned int height, unsigned int depth)
{
    if (depth < 1) {
        return error("unsupported image depth, try a different image");
    }
    if (SIZE_MAX / (size_t) width / height <= depth || width < 2 || height < 2) {
        return error("target dimensions are too small, try a different image");
    }
    image->buf_len = (size_t) width * height * depth;
    if ((image->buf = malloc(image->buf_len)) == NULL) {
        return error("out of memory - try with a smaller image");
    }
    image->width      = width;
    image->height     = height;
    image->depth      = depth;
    image->row_stride = (unsigned long) image->width * image->depth;

    return 0;
}

// Deallocate an image structure
static void image_free(Image *image)
{
    free(image->buf);
    image->buf     = NULL;
    image->buf_len = 0;
}

// Decompress a JPEG image in memory
static int jpeg_decompress(Image *image, const uint8_t *jpeg_buf, size_t jpeg_len)
{
    struct jpeg_decompress_struct dinfo;
    struct jpeg_error_mgr         jerr;

    dinfo.err = jpeg_std_error(&jerr);
    jpeg_create_decompress(&dinfo);
    jpeg_mem_src(&dinfo, jpeg_buf, jpeg_len);
    if (jpeg_read_header(&dinfo, TRUE) != 1) {
        return error("Invalid JPEG file");
    }
    jpeg_start_decompress(&dinfo);
    if (image_new(image, dinfo.output_width, dinfo.output_height, dinfo.output_components) != 0) {
        return -1;
    }
    while (dinfo.output_scanline < dinfo.output_height) {
        unsigned char *buffer_array;
        buffer_array = image->buf + dinfo.output_scanline * image->row_stride;
        if (jpeg_read_scanlines(&dinfo, &buffer_array, 1) != 1) {
            return error("Invalid JPEG file");
        }
    }
    jpeg_finish_decompress(&dinfo);
    jpeg_destroy_decompress(&dinfo);

    return 0;
}

// Compress a JPEG image in memory
static int jpeg_compress(uint8_t **jpeg_buf_p, size_t *jpeg_len_p, const Image *image)
{
    struct jpeg_compress_struct cinfo;
    struct jpeg_error_mgr       jerr;
    unsigned long               jpeg_len_ulong = 0;

    *jpeg_buf_p = NULL;
    *jpeg_len_p = 0;
    cinfo.err   = jpeg_std_error(&jerr);
    jpeg_create_compress(&cinfo);
    jpeg_mem_dest(&cinfo, jpeg_buf_p, &jpeg_len_ulong);
    cinfo.image_width      = image->width;
    cinfo.image_height     = image->height;
    cinfo.input_components = image->depth;
    cinfo.in_color_space   = JCS_RGB;
    jpeg_set_defaults(&cinfo);
    cinfo.progressive_mode = TRUE;
    jpeg_start_compress(&cinfo, TRUE);
    while (cinfo.next_scanline < cinfo.image_height) {
        unsigned char *buffer_array;
        buffer_array = image->buf + cinfo.next_scanline * image->row_stride;
        jpeg_write_scanlines(&cinfo, &buffer_array, 1);
    }
    jpeg_finish_compress(&cinfo);
    jpeg_destroy_compress(&cinfo);
    *jpeg_len_p = (size_t) jpeg_len_ulong;

    return 0;
}

// HTTP query handler
void run(void)
{
    request_t    outgoing_req;
    response_t   jpeg_bin_resp;
    uint8_t *    jpeg_buf;
    size_t       jpeg_len;
    unsigned int new_width  = 0;
    unsigned int new_height = 0;
    unsigned int opacity    = 0;

    hostcall_init_mm(guest_malloc, guest_free);

    // Read the `width`, `height`, and `sharpening` HTTP parameters
    get_query_ival(&new_width, "width");
    get_query_ival(&new_height, "height");
    get_query_ival(&opacity, "sharpening");
    if (new_width < 2 || new_height < 2 || opacity > 100) {
        const char *help =
            "<!doctype html><html><head><meta charset=utf-8><title>Terrarium image processing "
            "example</title></head><body>"
            "<h1>Terrarium image processing example</h1>"
            "<p>An image is retrieved from a remote URL, decompressed, resized, sharpened and "
            "recompressed on the fly.</p>"
            "<p>The image URL can be changed in the <code>image.h</code> header file.<p>"
            "<p>HTTP query parameters affecting the returned images are: <code>width</code> "
            "(pixels), "
            "<code>height</code> (pixels) and <code>sharpening</code> (0=no sharpening, 100=max "
            "sharpening).</p>"
            "<p>Example of dynamically generated image URLs:</p>"
            "<ul>"
            "<li><a href='/?width=640&height=360'>640x360, no sharpening</a>"
            "<li><a href='/?width=640&height=360&sharpening=25'>640x360, 25% sharpening</a>"
            "<li><a href='/?width=1280&height=720&sharpening=20'>1280x720, 20% sharpening</a>"
            "<li><a href='/?width=320&height=240&sharpening=10'>320x240, 10% sharpening</a>"
            "</ul>"
            "</body></html>";
        set_resp_http_header(RESPONSE_OUTGOING, "Content-Type", "text/html");
        hostcall_resp_set_body(RESPONSE_OUTGOING, help, strlen(help));
        set_nocache(RESPONSE_OUTGOING);
        hostcall_resp_set_response_code(RESPONSE_OUTGOING, 422);
        return;
    }

    // Retrieve the image from the local cache, or fetch it if absent
    if (hostcall_kvstore_get(&jpeg_buf, &jpeg_len, CACHE_KEY, sizeof CACHE_KEY - 1) == 0) {
#ifdef IMAGE_URI
        outgoing_req =
            hostcall_req_create("GET", sizeof "GET" - 1, IMAGE_URI, sizeof IMAGE_URI - 1);
#else
        char *image_uri = url_for_static_asset(LOCAL_IMAGE_URI);
        outgoing_req = hostcall_req_create("GET", sizeof "GET" - 1, image_uri, strlen(image_uri));
        free(image_uri);
#endif
        jpeg_bin_resp = hostcall_req_send(outgoing_req);
        hostcall_resp_get_body(&jpeg_buf, &jpeg_len, jpeg_bin_resp);
        hostcall_kvstore_insert(CACHE_KEY, sizeof CACHE_KEY - 1, (const void *) jpeg_buf, jpeg_len);
    }

    // Decompress the image
    Image in;
    if (jpeg_decompress(&in, jpeg_buf, jpeg_len) != 0) {
        return;
    }
    free(jpeg_buf);

    // Resize the image
    Image resized;
    if (image_new(&resized, new_width, new_height, in.depth) != 0) {
        return;
    }
    resize(&resized, &in);
    image_free(&in);

    // Apply the kernel
    Image out;
    if (image_new(&out, resized.width, resized.height, resized.depth) != 0) {
        return;
    }
    convolution(&out, &resized);
    layer_merge(&out, &resized, opacity);
    image_free(&resized);

    // Compress the new image to JPEG
    uint8_t *recompressed_buf;
    size_t   recompressed_len;
    if (jpeg_compress(&recompressed_buf, &recompressed_len, &out) != 0) {
        return;
    }
    image_free(&out);

    // Serve the new image
    set_resp_http_header(RESPONSE_OUTGOING, "Content-Type", "image/jpeg");
    set_nocache(RESPONSE_OUTGOING);
    hostcall_resp_set_body(RESPONSE_OUTGOING, (const void *) recompressed_buf, recompressed_len);
}
