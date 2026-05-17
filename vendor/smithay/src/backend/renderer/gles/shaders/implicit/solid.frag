#version 100

precision mediump float;
uniform vec4 color;

void main() {
    // A18.2: pow() em premul gera cor errada nas bordas AA. Demultiplicar,
    // aplicar gamma, remultiplicar (matematica correta pra premul + sRGB).
    vec3 srgb_rgb;
    if (color.a > 0.0001) {
        vec3 linear_rgb = color.rgb / color.a;
        srgb_rgb = pow(linear_rgb, vec3(1.0/2.2)) * color.a;
    } else {
        srgb_rgb = vec3(0.0);
    }
    gl_FragColor = vec4(srgb_rgb, color.a);
}