/* @ts-self-types="./modelist_wasm.d.ts" */

export class ModelistSession {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ModelistSessionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_modelistsession_free(ptr, 0);
    }
    /**
     * Adds a garment drafted from a template, to the right of the others on the canvas and
     * one layer above them in the 3D fit. It becomes the active garment. `name` (the template
     * name in the UI language) is optional. Returns its id.
     * @param {string} template_id
     * @param {string} name
     * @returns {string}
     */
    add_component(template_id, name) {
        let deferred3_0;
        let deferred3_1;
        try {
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(template_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.modelistsession_add_component(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} piece_id
     * @param {number} x
     * @param {number} y
     * @param {string} label
     */
    add_drill_hole(piece_id, x, y, label) {
        const val3 = +x;
        const val4 = +y;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_add_drill_hole(this.__wbg_ptr, ptr0, len0, val3, val4, ptr1, len1);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} param_t
     * @param {string} style
     * @param {number} count
     * @param {number} depth_mm
     */
    add_notch(piece_id, edge_idx, param_t, style, count, depth_mm) {
        const val3 = +edge_idx;
        const val4 = +param_t;
        const val5 = +count;
        const val6 = +depth_mm;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(style, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_add_notch(this.__wbg_ptr, ptr0, len0, val3, val4, ptr1, len1, val5, val6);
    }
    /**
     * @param {string} name
     * @param {number} x0
     * @param {number} y0
     * @param {number} x1
     * @param {number} y1
     */
    add_rect_piece(name, x0, y0, x1, y1) {
        const val2 = +x0;
        const val3 = +y0;
        const val4 = +x1;
        const val5 = +y1;
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_add_rect_piece(this.__wbg_ptr, ptr0, len0, val2, val3, val4, val5);
    }
    /**
     * @returns {number}
     */
    auto_detect_seams() {
        const ret = wasm.modelistsession_auto_detect_seams(this.__wbg_ptr);
        return ret >>> 0;
    }
    clear_seam_walk() {
        wasm.modelistsession_clear_seam_walk(this.__wbg_ptr);
    }
    /**
     * @param {string} config_json
     * @returns {string}
     */
    compute_marker_plan_json(config_json) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(config_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_compute_marker_plan_json(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {string} link_id
     * @param {number} progress_u
     * @returns {string}
     */
    compute_seam_walk(link_id, progress_u) {
        let deferred3_0;
        let deferred3_1;
        try {
            const val2 = +progress_u;
            const ptr0 = passStringToWasm0(link_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_compute_seam_walk(this.__wbg_ptr, ptr0, len0, val2);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {boolean} to_cubic
     */
    convert_edge_curve(piece_id, edge_idx, to_cubic) {
        const val2 = +edge_idx;
        const val3 = +to_cubic;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_convert_edge_curve(this.__wbg_ptr, ptr0, len0, val2, val3);
    }
    /**
     * @param {string} label
     * @param {string} piece_a_id
     * @param {string} edge_a_id
     * @param {string} piece_b_id
     * @param {string} edge_b_id
     * @param {string} driver
     * @param {string} ease_json
     */
    create_seam_binding(label, piece_a_id, edge_a_id, piece_b_id, edge_b_id, driver, ease_json) {
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6;
        try {
            ptr0 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(piece_a_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(edge_a_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(piece_b_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
            ptr4 = passStringToWasm0(edge_b_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len4 = WASM_VECTOR_LEN;
            ptr5 = passStringToWasm0(driver, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len5 = WASM_VECTOR_LEN;
            ptr6 = passStringToWasm0(ease_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len6 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr6) { wasm.__wbindgen_free(ptr6, len6 * 1, 1); }
            if (ptr5) { wasm.__wbindgen_free(ptr5, len5 * 1, 1); }
            if (ptr4) { wasm.__wbindgen_free(ptr4, len4 * 1, 1); }
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_create_seam_binding(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6);
    }
    /**
     * @param {string} binding_id
     */
    delete_seam_binding(binding_id) {
        const ptr0 = passStringToWasm0(binding_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_delete_seam_binding(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} name
     */
    delete_variable(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_delete_variable(this.__wbg_ptr, ptr0, len0);
    }
    dispose_sim3d() {
        wasm.modelistsession_dispose_sim3d(this.__wbg_ptr);
    }
    /**
     * Adds a copy of a garment (same template, ease, formulas, fabrics and history) beside the
     * others, one layer up. Returns the copy's id.
     * @param {string} component_id
     * @returns {string}
     */
    duplicate_component(component_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_duplicate_component(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {string} piece_id
     */
    duplicate_piece(piece_id) {
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_duplicate_piece(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @returns {string}
     */
    export_aama_rul() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_export_aama_rul(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * One garment as its own `.modelist` file (for the garment library or to share), on the
     * current body. `thumbnail_png` may be empty.
     * @param {string} component_id
     * @param {Uint8Array | number[]} thumbnail_png
     * @returns {Uint8Array}
     */
    export_component_file(component_id, thumbnail_png) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passArray8ToWasm0(thumbnail_png, wasm.__wbindgen_malloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.modelistsession_export_component_file(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v3;
    }
    /**
     * @returns {string}
     */
    export_dxf_aama() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_export_dxf_aama(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} config_json
     * @returns {string}
     */
    export_marker_svg(config_json) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(config_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_export_marker_svg(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    export_pom_csv() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_export_pom_csv(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {boolean} include_nest
     * @returns {string}
     */
    export_svg_mm(include_nest) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_export_svg_mm(this.__wbg_ptr, include_nest);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Float32Array}
     */
    get_handle_points() {
        const ret = wasm.modelistsession_get_handle_points(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} preset
     * @param {boolean} unfold_on_fold
     * @returns {string}
     */
    get_pdf_layout_json(preset, unfold_on_fold) {
        let deferred3_0;
        let deferred3_1;
        try {
            const val2 = +unfold_on_fold;
            const ptr0 = passStringToWasm0(preset, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_get_pdf_layout_json(this.__wbg_ptr, ptr0, len0, val2);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @returns {Uint32Array}
     */
    get_sim3d_avatar_indices() {
        const ret = wasm.modelistsession_get_sim3d_avatar_indices(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_avatar_normals() {
        const ret = wasm.modelistsession_get_sim3d_avatar_normals(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_avatar_positions() {
        const ret = wasm.modelistsession_get_sim3d_avatar_positions(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_avatar_torso_mask() {
        const ret = wasm.modelistsession_get_sim3d_avatar_torso_mask(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_cloth_clearance() {
        const ret = wasm.modelistsession_get_sim3d_cloth_clearance(this.__wbg_ptr);
        return ret;
    }
    /**
     * Per-vertex cloth colour (rgb 0..1) from each piece role's fabric.
     * @returns {Float32Array}
     */
    get_sim3d_cloth_colors() {
        const ret = wasm.modelistsession_get_sim3d_cloth_colors(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Uint32Array}
     */
    get_sim3d_cloth_indices() {
        const ret = wasm.modelistsession_get_sim3d_cloth_indices(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_cloth_normals() {
        const ret = wasm.modelistsession_get_sim3d_cloth_normals(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_cloth_positions() {
        const ret = wasm.modelistsession_get_sim3d_cloth_positions(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_cloth_strain() {
        const ret = wasm.modelistsession_get_sim3d_cloth_strain(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    get_sim3d_cloth_tension() {
        const ret = wasm.modelistsession_get_sim3d_cloth_tension(this.__wbg_ptr);
        return ret;
    }
    /**
     * Drape-quality metrics (poke-through, creases, self-intersections) as JSON. A few ms:
     * poll it occasionally, not every frame.
     * @returns {string}
     */
    get_sim3d_quality_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_get_sim3d_quality_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Float32Array}
     */
    get_stroke_coords() {
        const ret = wasm.modelistsession_get_stroke_coords(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Uint32Array}
     */
    get_stroke_meta() {
        const ret = wasm.modelistsession_get_stroke_meta(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    get_ui_snapshot_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_get_ui_snapshot_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} wx
     * @param {number} wy
     * @param {number} world_tol_mm
     * @returns {string}
     */
    hit_test(wx, wy, world_tol_mm) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_hit_test(this.__wbg_ptr, wx, wy, world_tol_mm);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} dxf_text
     * @returns {string}
     */
    import_dxf_aama(dxf_text) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(dxf_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_import_dxf_aama(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} json_text
     * @returns {string}
     */
    import_garmentcode_json(json_text) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(json_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_import_garmentcode_json(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} pose
     * @param {string} body_form
     * @returns {string}
     */
    init_sim3d(pose, body_form) {
        let deferred3_0;
        let deferred3_1;
        try {
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(pose, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(body_form, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.modelistsession_init_sim3d(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Adds the garments of a `.modelist` file (a saved garment, or every garment of another
     * project) beside the others. They are refitted to this document's body. Returns the id of
     * the last one added (it becomes the active garment).
     * @param {Uint8Array | number[]} bytes
     * @returns {string}
     */
    insert_component_file(bytes) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_insert_component_file(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Opens a project file: a `.modelist` container, or plain JSON (`.modelist.json`, any
     * format version). Returns the saved UI state (`session.json`), or an empty string.
     * @param {Uint8Array | number[]} bytes
     * @returns {string}
     */
    load_project_file(bytes) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.modelistsession_load_project_file(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} json
     */
    load_project_json(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.modelistsession_load_project_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} template_id
     */
    load_template(template_id) {
        const ptr0 = passStringToWasm0(template_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_load_template(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} piece_id
     */
    mirror_piece(piece_id) {
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_mirror_piece(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Moves a garment one place outward (worn over the next garment) or inward, swapping
     * layers with its neighbour; layers are then renumbered 0, 1, 2... in wearing order.
     * @param {string} component_id
     * @param {boolean} outward
     */
    move_component_layer(component_id, outward) {
        const val2 = +outward;
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_move_component_layer(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} which
     * @param {number} total_dx
     * @param {number} total_dy
     * @param {boolean} lock_g1
     * @param {boolean} commit
     */
    move_handle(piece_id, edge_idx, which, total_dx, total_dy, lock_g1, commit) {
        const val2 = +edge_idx;
        const val3 = +which;
        const val4 = +total_dx;
        const val5 = +total_dy;
        const val6 = +lock_g1;
        const val7 = +commit;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_move_handle(this.__wbg_ptr, ptr0, len0, val2, val3, val4, val5, val6, val7);
    }
    /**
     * @param {string} piece_id
     * @param {number} total_dx
     * @param {number} total_dy
     * @param {boolean} commit
     */
    move_piece(piece_id, total_dx, total_dy, commit) {
        const val2 = +total_dx;
        const val3 = +total_dy;
        const val4 = +commit;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_move_piece(this.__wbg_ptr, ptr0, len0, val2, val3, val4);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} total_dx
     * @param {number} total_dy
     * @param {boolean} commit
     */
    move_vertex(piece_id, edge_idx, total_dx, total_dy, commit) {
        const val2 = +edge_idx;
        const val3 = +total_dx;
        const val4 = +total_dy;
        const val5 = +commit;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_move_vertex(this.__wbg_ptr, ptr0, len0, val2, val3, val4, val5);
    }
    /**
     * @param {string} template_id
     */
    constructor(template_id) {
        const ptr0 = passStringToWasm0(template_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.modelistsession_new(ptr0, len0);
        this.__wbg_ptr = ret;
        ModelistSessionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {string} piece_id
     * @param {string} dart_id
     * @param {number} target_edge_idx
     * @param {number} target_t
     * @param {number} angle_deg
     */
    pivot_dart(piece_id, dart_id, target_edge_idx, target_t, angle_deg) {
        const val3 = +target_edge_idx;
        const val4 = +target_t;
        const val5 = +angle_deg;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(dart_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_pivot_dart(this.__wbg_ptr, ptr0, len0, ptr1, len1, val3, val4, val5);
    }
    /**
     * @param {number} wx
     * @param {number} wy
     * @param {number} world_tol_mm
     * @returns {string}
     */
    query_snap(wx, wy, world_tol_mm) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_query_snap(this.__wbg_ptr, wx, wy, world_tol_mm);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {boolean}
     */
    redo() {
        const ret = wasm.modelistsession_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Removes a garment (the last one cannot be removed).
     * @param {string} component_id
     */
    remove_component(component_id) {
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_remove_component(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} op_id
     */
    remove_op(op_id) {
        const ptr0 = passStringToWasm0(op_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_remove_op(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} component_id
     * @param {string} name
     */
    rename_component(component_id, name) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_rename_component(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {number} from_idx
     * @param {number} to_idx
     * @returns {boolean}
     */
    reorder_op(from_idx, to_idx) {
        const ret = wasm.modelistsession_reorder_op(this.__wbg_ptr, from_idx, to_idx);
        return ret !== 0;
    }
    /**
     * Restores the shell fabric's physical properties from its preset (keeps the colour).
     */
    reset_fabric_to_preset() {
        wasm.modelistsession_reset_fabric_to_preset(this.__wbg_ptr);
    }
    /**
     * @param {string} pose
     * @param {string} body_form
     * @returns {string}
     */
    reset_sim3d(pose, body_form) {
        let deferred3_0;
        let deferred3_1;
        try {
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(pose, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(body_form, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.modelistsession_reset_sim3d(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Writes the project as a `.modelist` container (ZIP of JSON parts, see
     * `modelist_core::container`). `modified` is the save time (ISO 8601) and `session_json`
     * the UI state to restore; both may be empty, like `thumbnail_png`. Parts of the opened
     * file this build does not know are written back unchanged.
     * @param {string} modified
     * @param {string} session_json
     * @param {Uint8Array | number[]} thumbnail_png
     * @returns {Uint8Array}
     */
    save_project_container(modified, session_json, thumbnail_png) {
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(modified, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(session_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passArray8ToWasm0(thumbnail_png, wasm.__wbindgen_malloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.modelistsession_save_project_container(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v4;
    }
    /**
     * @returns {string}
     */
    serialize_project_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_serialize_project_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Makes a garment the one edits, parameters, fabrics and the timeline apply to.
     * @param {string} component_id
     */
    set_active_component(component_id) {
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_active_component(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} size
     */
    set_active_size(size) {
        const ptr0 = passStringToWasm0(size, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_active_size(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Garment layer in the 3D fit (0 = next to the body). Re-dresses the 3D fit when open.
     * @param {string} component_id
     * @param {number} layer
     */
    set_component_layer(component_id, layer) {
        const val2 = +layer;
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_component_layer(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * Shows or hides a garment in the 3D fit only. A hidden garment is still worn, so hiding
     * an outer layer reveals what is under it without re-draping anything.
     * @param {string} component_id
     * @param {boolean} visible
     */
    set_component_visible_3d(component_id, visible) {
        const val2 = +visible;
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_component_visible_3d(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * Shows or hides a garment everywhere (canvas, exports, 3D fit).
     * @param {string} component_id
     * @param {boolean} visible
     */
    set_component_visible(component_id, visible) {
        const val2 = +visible;
        const ptr0 = passStringToWasm0(component_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_component_visible(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * @param {string} key
     * @param {number} val
     */
    set_ease(key, val) {
        const val2 = +val;
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_ease(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} target_len_mm
     */
    set_edge_length(piece_id, edge_idx, target_len_mm) {
        const val2 = +edge_idx;
        const val3 = +target_len_mm;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_edge_length(this.__wbg_ptr, ptr0, len0, val2, val3);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} width_mm
     * @param {string} corner_rule_json
     */
    set_edge_sa(piece_id, edge_idx, width_mm, corner_rule_json) {
        const val3 = +edge_idx;
        const val4 = +width_mm;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(corner_rule_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_edge_sa(this.__wbg_ptr, ptr0, len0, val3, val4, ptr1, len1);
    }
    /**
     * @param {string} color_hex
     */
    set_fabric_color(color_hex) {
        const ptr0 = passStringToWasm0(color_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_fabric_color(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Surface finish for the 3D render: plain, twill, satin, jersey, rib, sheer, terry.
     * @param {string} finish
     */
    set_fabric_finish(finish) {
        const ptr0 = passStringToWasm0(finish, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_fabric_finish(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} preset_id
     */
    set_fabric_preset(preset_id) {
        const ptr0 = passStringToWasm0(preset_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_fabric_preset(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Sets one physical property of the shell fabric (key from `fabric_property_specs_json`).
     * Values are clamped to the property's valid range. Returns false for unknown keys.
     * @param {string} key
     * @param {number} value
     * @returns {boolean}
     */
    set_fabric_property(key, value) {
        const val2 = +value;
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.modelistsession_set_fabric_property(this.__wbg_ptr, ptr0, len0, val2);
        return ret !== 0;
    }
    /**
     * @param {string} mode
     */
    set_grade_mode(mode) {
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_grade_mode(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} piece_id
     * @param {string} mode
     */
    set_grain_rotation(piece_id, mode) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_grain_rotation(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {string} key
     * @param {number} val
     */
    set_measurement(key, val) {
        const val2 = +val;
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_measurement(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * @param {string} name
     */
    set_project_name(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_project_name(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} role
     * @param {string} color_hex
     */
    set_role_fabric_color(role, color_hex) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(role, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(color_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_role_fabric_color(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Assigns a preset to a piece role (e.g. "rib"); an empty preset id means "same as shell".
     * @param {string} role
     * @param {string} preset_id
     */
    set_role_fabric_preset(role, preset_id) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(role, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(preset_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_role_fabric_preset(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Sets one physical property of a role's fabric (the role must have its own fabric).
     * @param {string} role
     * @param {string} key
     * @param {number} value
     * @returns {boolean}
     */
    set_role_fabric_property(role, key, value) {
        const val3 = +value;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(role, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.modelistsession_set_role_fabric_property(this.__wbg_ptr, ptr0, len0, ptr1, len1, val3);
        return ret !== 0;
    }
    /**
     * @param {number} index
     */
    set_rollback_index(index) {
        wasm.modelistsession_set_rollback_index(this.__wbg_ptr, index);
    }
    /**
     * @param {string} edges_a_json
     * @param {string} edges_b_json
     */
    set_seam_draft_preview(edges_a_json, edges_b_json) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(edges_a_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(edges_b_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_seam_draft_preview(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {string | null} [binding_id]
     */
    set_selected_binding(binding_id) {
        var ptr0 = isLikeNone(binding_id) ? 0 : passStringToWasm0(binding_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_selected_binding(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string | null} [piece_id]
     * @param {number | null} [edge_idx]
     */
    set_selection(piece_id, edge_idx) {
        const val2 = +(isLikeNone(edge_idx) ? Number.MAX_SAFE_INTEGER : (edge_idx) >>> 0);
        var ptr0 = isLikeNone(piece_id) ? 0 : passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_selection(this.__wbg_ptr, ptr0, len0, val2);
    }
    /**
     * @param {boolean} show
     */
    set_show_mirror_unfold(show) {
        wasm.modelistsession_set_show_mirror_unfold(this.__wbg_ptr, show);
    }
    /**
     * @param {string} anchor
     */
    set_stack_anchor(anchor) {
        const ptr0 = passStringToWasm0(anchor, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_stack_anchor(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} name
     * @param {string} expr
     * @param {string} description
     */
    set_variable(name, expr, description) {
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(expr, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(description, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_set_variable(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
    }
    /**
     * @param {string} sizes_json
     */
    set_visible_sizes(sizes_json) {
        const ptr0 = passStringToWasm0(sizes_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_set_visible_sizes(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} piece_id
     * @param {number} start_edge_idx
     * @param {number} end_edge_idx
     * @param {number} slash_count
     * @param {number} spread_mm
     * @param {string} hinge_mode
     */
    slash_and_spread(piece_id, start_edge_idx, end_edge_idx, slash_count, spread_mm, hinge_mode) {
        const val3 = +start_edge_idx;
        const val4 = +end_edge_idx;
        const val5 = +slash_count;
        const val6 = +spread_mm;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(hinge_mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_slash_and_spread(this.__wbg_ptr, ptr0, len0, val3, val4, val5, val6, ptr1, len1);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {number} param_t
     */
    split_edge_point(piece_id, edge_idx, param_t) {
        const val2 = +edge_idx;
        const val3 = +param_t;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_split_edge_point(this.__wbg_ptr, ptr0, len0, val2, val3);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_a_idx
     * @param {number} t_a
     * @param {number} edge_b_idx
     * @param {number} t_b
     * @param {string} label_a
     * @param {string} label_b
     */
    split_style_line(piece_id, edge_a_idx, t_a, edge_b_idx, t_b, label_a, label_b) {
        const val4 = +edge_a_idx;
        const val5 = +t_a;
        const val6 = +edge_b_idx;
        const val7 = +t_b;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(label_a, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(label_b, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_split_style_line(this.__wbg_ptr, ptr0, len0, val4, val5, val6, val7, ptr1, len1, ptr2, len2);
    }
    /**
     * @param {number} steps
     * @returns {string}
     */
    step_sim3d(steps) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelistsession_step_sim3d(this.__wbg_ptr, steps);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} piece_id
     */
    toggle_piece_visibility(piece_id) {
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_toggle_piece_visibility(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} op_id
     */
    toggle_suppress_op(op_id) {
        const ptr0 = passStringToWasm0(op_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_toggle_suppress_op(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {string} piece_id
     * @param {number} edge_idx
     * @param {boolean} square_90
     */
    true_corner(piece_id, edge_idx, square_90) {
        const val2 = +edge_idx;
        const val3 = +square_90;
        const ptr0 = passStringToWasm0(piece_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.modelistsession_true_corner(this.__wbg_ptr, ptr0, len0, val2, val3);
    }
    /**
     * @returns {boolean}
     */
    undo() {
        const ret = wasm.modelistsession_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {string} op_id
     * @param {string} patch_json
     */
    update_op_json(op_id, patch_json) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(op_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(patch_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.modelistsession_update_op_json(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} binding_id
     * @param {string} driver
     * @param {string} ease_json
     * @param {string} seam_type
     * @param {boolean} auto_partner_notches
     */
    update_seam_binding(binding_id, driver, ease_json, seam_type, auto_partner_notches) {
        const val5 = +auto_partner_notches;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3;
        try {
            ptr0 = passStringToWasm0(binding_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(driver, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(ease_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(seam_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        wasm.modelistsession_update_seam_binding(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, val5);
    }
    /**
     * Creates or replaces a full multi-edge SeamBinding (used by the Fusion 360 Seam Command Dialog).
     * @param {string | null | undefined} binding_id
     * @param {string} label
     * @param {string} edges_a_json
     * @param {string} edges_b_json
     * @param {string} driver
     * @param {string} ease_json
     * @param {string} seam_type
     * @param {boolean} auto_partner_notches
     * @param {boolean} sync_recommended_sa
     * @returns {string}
     */
    upsert_seam_binding(binding_id, label, edges_a_json, edges_b_json, driver, ease_json, seam_type, auto_partner_notches, sync_recommended_sa) {
        let deferred11_0;
        let deferred11_1;
        try {
            const val8 = +auto_partner_notches;
            const val9 = +sync_recommended_sa;
            let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6;
            try {
                ptr0 = isLikeNone(binding_id) ? 0 : passStringToWasm0(binding_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
                ptr2 = passStringToWasm0(edges_a_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len2 = WASM_VECTOR_LEN;
                ptr3 = passStringToWasm0(edges_b_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len3 = WASM_VECTOR_LEN;
                ptr4 = passStringToWasm0(driver, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len4 = WASM_VECTOR_LEN;
                ptr5 = passStringToWasm0(ease_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len5 = WASM_VECTOR_LEN;
                ptr6 = passStringToWasm0(seam_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len6 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr6) { wasm.__wbindgen_free(ptr6, len6 * 1, 1); }
                if (ptr5) { wasm.__wbindgen_free(ptr5, len5 * 1, 1); }
                if (ptr4) { wasm.__wbindgen_free(ptr4, len4 * 1, 1); }
                if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
                if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.modelistsession_upsert_seam_binding(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, val8, val9);
            var ptr10 = ret[0];
            var len10 = ret[1];
            if (ret[3]) {
                ptr10 = 0; len10 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred11_0 = ptr10;
            deferred11_1 = len10;
            return getStringFromWasm0(ptr10, len10);
        } finally {
            wasm.__wbindgen_free(deferred11_0, deferred11_1, 1);
        }
    }
}
if (Symbol.dispose) ModelistSession.prototype[Symbol.dispose] = ModelistSession.prototype.free;

/**
 * Editable fabric properties (keys, units, ranges, help) for the fabric editor.
 * @returns {string}
 */
export function fabric_property_specs_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.fabric_property_specs_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Starter library catalog (blocks + styles) as JSON, so the UI never hard-codes template ids.
 * @returns {string}
 */
export function template_catalog_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.template_catalog_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_getRandomValues_8b6f1f663e6f188d: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_new_from_slice_3512352010266e02: function(arg0, arg1) {
            const ret = new Uint32Array(getArrayU32FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_from_slice_355c10c5d8f2addb: function(arg0, arg1) {
            const ret = new Float32Array(getArrayF32FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_with_length_00818aa0a226a550: function(arg0) {
            const ret = new Uint32Array(arg0 >>> 0);
            return ret;
        },
        __wbg_new_with_length_5c0c9043ebd9ec32: function(arg0) {
            const ret = new Float32Array(arg0 >>> 0);
            return ret;
        },
        __wbg_now_463ac581d2fc7ea3: function() {
            const ret = Date.now();
            return ret;
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./modelist_wasm_bg.js": import0,
    };
}

const ModelistSessionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_modelistsession_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('modelist_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
