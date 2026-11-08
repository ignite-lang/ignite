target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'golden.capabilities'
source_filename = "golden.capabilities"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.4 = private unnamed_addr constant [2 x i8] c"-c", align 1
@.str.0 = private unnamed_addr constant [5 x i8] c"bench", align 1
@.str.1 = private unnamed_addr constant [6 x i8] c"in.txt", align 1
@.str.3 = private unnamed_addr constant [4 x i8] c"made", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"out.txt", align 1
@.str.5 = private unnamed_addr constant [4 x i8] c"true", align 1

declare void @ignite_rt_init(i32, ptr)
declare ptr @ignite_rt_alloc(i64, i64)
declare i8 @ignite_rt_spawn(ptr, ptr)
declare void @ignite_rt_spawn_ready()
declare void @ignite_rt_task_end()
declare i64 @ignite_rt_arena_mark()
declare void @ignite_rt_arena_release(i64)
declare i8 @ignite_rt_arena_due(i64, i64)
declare void @ignite_rt_arena_scratch(i8)
declare void @ignite_rt_arena_scratch_clear()
declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i32 @ignite_rt_args_port(ptr, ptr, ptr)
declare void @ignite_rt_exit(i32) noreturn
declare void @ignite_rt_fail(ptr) cold noreturn
declare void @ignite_rt_log(ptr)
declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)
declare i64 @ignite_rt_tcp_local_port(i64)
declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @ignite_rt_tcp_close(i64, ptr)
declare void @ignite_rt_tcp_release(i64)
declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)
declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)
declare i32 @ignite_rt_stdin_set_raw(i8, ptr)
declare i32 @ignite_rt_stdout_write_all(ptr, ptr)
declare i32 @ignite_rt_stderr_write_all(ptr, ptr)
declare void @ignite_rt_io_error_new(ptr, ptr)
declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)
declare void @ignite_rt_text_trim(ptr, ptr)
declare void @ignite_rt_text_concat(ptr, ptr, ptr)
declare i8 @ignite_rt_text_eq(ptr, ptr)
declare i8 @ignite_rt_text_starts_with(ptr, ptr)
declare void @ignite_rt_text_join(ptr, ptr, ptr)
declare void @ignite_rt_int_to_text(ptr, i64)
declare i8 @ignite_rt_int_parse(ptr, ptr)
declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @ignite_rt_store_next_id(ptr)
declare void @ignite_rt_store_put(ptr, i64, ptr)
declare i8 @ignite_rt_store_get(ptr, i64, ptr)
declare i8 @ignite_rt_store_remove(ptr, i64, ptr)
declare void @ignite_rt_store_values(ptr, ptr)
declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare i64 @"ignite_rt_child_id"(i64)
declare i32 @"ignite_rt_child_kill"(i64, ptr)
declare i32 @"ignite_rt_child_wait"(i64, ptr, ptr)
declare i32 @"ignite_rt_directory_absolute_path"(i64, ptr, ptr)
declare i32 @"ignite_rt_directory_create_directory"(i64, ptr, ptr)
declare i32 @"ignite_rt_directory_read_directory"(i64, ptr, ptr, ptr)
declare i32 @"ignite_rt_directory_read_file"(i64, ptr, ptr, ptr)
declare i32 @"ignite_rt_directory_remove"(i64, ptr, ptr)
declare i32 @"ignite_rt_directory_remove_tree"(i64, ptr, ptr)
declare i32 @"ignite_rt_directory_write_file"(i64, ptr, ptr, ptr)
declare i32 @"ignite_rt_entropy_random_bytes"(i64, i64, ptr, ptr)
declare i32 @"ignite_rt_launcher_execute"(i64, ptr, ptr, ptr)
declare i32 @"ignite_rt_launcher_start"(i64, ptr, ptr, ptr)
declare void @"ignite_rt_setting_value"(ptr, i64)
declare void @"std.process.Invocation.environment"(ptr)
declare i1 @"std.process.Invocation.inherit_environment"()
declare void @"std.process.Invocation.input"(ptr)
declare i64 @"std.process.Invocation.output_limit"()
declare i64 @"std.process.Invocation.timeout"()

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) alwaysinline {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  call void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align)
  br label %done
done:
  ret void
}

define internal void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align) noinline cold {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define void @"golden.capabilities.Bench.executable"(ptr %sret) !dbg !4 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t4 = alloca [24 x i8], align 8
  store ptr @.str.0, ptr %t1, align 8, !dbg !5
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !5
  store i64 5, ptr %t2, align 8, !dbg !5
  %t3 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !5
  store i64 5, ptr %t3, align 8, !dbg !5
  call void @"copy.Text"(ptr %t4, ptr %t1), !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t4, i64 24, i1 false), !dbg !5
  ret void, !dbg !5
}

define void @"golden.capabilities.Bench.main"(ptr %sret, ptr noalias readonly %p0) !dbg !6 {
entry:
  %t2 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  %t6 = alloca [40 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t8 = alloca [32 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t25 = alloca [24 x i8], align 8
  %t28 = alloca [24 x i8], align 8
  %t29 = alloca [40 x i8], align 8
  %t30 = alloca [32 x i8], align 8
  %t45 = alloca [24 x i8], align 8
  %t48 = alloca [24 x i8], align 8
  %t49 = alloca [40 x i8], align 8
  %t50 = alloca [24 x i8], align 8
  %t51 = alloca [32 x i8], align 8
  %t66 = alloca [24 x i8], align 8
  %t68 = alloca [24 x i8], align 8
  %t71 = alloca [24 x i8], align 8
  %t72 = alloca [40 x i8], align 8
  %t73 = alloca [32 x i8], align 8
  %t88 = alloca [24 x i8], align 8
  %t91 = alloca [24 x i8], align 8
  %t92 = alloca [40 x i8], align 8
  %t93 = alloca [32 x i8], align 8
  %t108 = alloca [24 x i8], align 8
  %t111 = alloca [24 x i8], align 8
  %t112 = alloca [40 x i8], align 8
  %t113 = alloca [32 x i8], align 8
  %t128 = alloca [40 x i8], align 8
  %t129 = alloca [24 x i8], align 8
  %t130 = alloca [32 x i8], align 8
  %t145 = alloca [24 x i8], align 8
  %t146 = alloca [144 x i8], align 8
  %t149 = alloca [24 x i8], align 8
  %t151 = alloca [24 x i8], align 8
  %t154 = alloca [24 x i8], align 8
  %t155 = alloca [24 x i8], align 8
  %t158 = alloca [24 x i8], align 8
  %t160 = alloca [24 x i8], align 8
  %t165 = alloca [24 x i8], align 8
  %t174 = alloca [24 x i8], align 8
  %t176 = alloca [144 x i8], align 8
  %t179 = alloca [80 x i8], align 8
  %t180 = alloca [72 x i8], align 8
  %t181 = alloca [32 x i8], align 8
  %t196 = alloca [72 x i8], align 8
  %t199 = alloca [40 x i8], align 8
  %t200 = alloca [8 x i8], align 8
  %t201 = alloca [32 x i8], align 8
  %t217 = alloca [8 x i8], align 8
  %t220 = alloca [8 x i8], align 8
  %t222 = alloca [40 x i8], align 8
  %t223 = alloca [32 x i8], align 8
  %t238 = alloca [80 x i8], align 8
  %t239 = alloca [72 x i8], align 8
  %t240 = alloca [32 x i8], align 8
  %t255 = alloca [72 x i8], align 8
  %t258 = alloca [40 x i8], align 8
  %t259 = alloca [24 x i8], align 8
  %t260 = alloca [32 x i8], align 8
  %t275 = alloca [24 x i8], align 8
  %t276 = alloca [40 x i8], align 8
  %t1 = load i64, ptr %p0, align 8, !dbg !9
  store ptr @.str.1, ptr %t2, align 8, !dbg !10
  %t3 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !10
  store i64 6, ptr %t3, align 8, !dbg !10
  %t4 = getelementptr inbounds i8, ptr %t2, i64 16, !dbg !10
  store i64 6, ptr %t4, align 8, !dbg !10
  call void @"copy.Text"(ptr %t5, ptr %t2), !dbg !10
  %t9 = call i32 @ignite_rt_directory_read_file(i64 %t1, ptr %t5, ptr %t7, ptr %t8), !dbg !9
  %t10 = icmp ne i32 %t9, 0, !dbg !9
  br i1 %t10, label %L11, label %L12, !dbg !9
L11:
  store i32 1, ptr %t6, align 4, !dbg !9
  %t14 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !9
  call void @llvm.memcpy.p0.p0.i64(ptr %t14, ptr %t8, i64 32, i1 false), !dbg !9
  br label %L13, !dbg !9
L12:
  store i32 0, ptr %t6, align 4, !dbg !9
  %t15 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !9
  call void @llvm.memcpy.p0.p0.i64(ptr %t15, ptr %t7, i64 24, i1 false), !dbg !9
  br label %L13, !dbg !9
L13:
  %t16 = load i32, ptr %t6, align 4, !dbg !9
  %t17 = icmp eq i32 %t16, 1, !dbg !9
  br i1 %t17, label %L18, label %L19, !dbg !9
L18:
  store i32 1, ptr %sret, align 4, !dbg !9
  %t20 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !9
  %t21 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !9
  call void @llvm.memcpy.p0.p0.i64(ptr %t21, ptr %t20, i64 32, i1 false), !dbg !9
  ret void, !dbg !9
L19:
  %t22 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !9
  call void @llvm.memcpy.p0.p0.i64(ptr %t23, ptr %t22, i64 24, i1 false), !dbg !8
  %t24 = load i64, ptr %p0, align 8, !dbg !11
  store ptr @.str.2, ptr %t25, align 8, !dbg !12
  %t26 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !12
  store i64 7, ptr %t26, align 8, !dbg !12
  %t27 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !12
  store i64 7, ptr %t27, align 8, !dbg !12
  call void @"copy.Text"(ptr %t28, ptr %t25), !dbg !12
  %t31 = call i32 @ignite_rt_directory_write_file(i64 %t24, ptr %t28, ptr %t23, ptr %t30), !dbg !11
  %t32 = icmp ne i32 %t31, 0, !dbg !11
  br i1 %t32, label %L33, label %L34, !dbg !11
L33:
  store i32 1, ptr %t29, align 4, !dbg !11
  %t36 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !11
  call void @llvm.memcpy.p0.p0.i64(ptr %t36, ptr %t30, i64 32, i1 false), !dbg !11
  br label %L35, !dbg !11
L34:
  store i32 0, ptr %t29, align 4, !dbg !11
  br label %L35, !dbg !11
L35:
  %t37 = load i32, ptr %t29, align 4, !dbg !11
  %t38 = icmp eq i32 %t37, 1, !dbg !11
  br i1 %t38, label %L39, label %L40, !dbg !11
L39:
  store i32 1, ptr %sret, align 4, !dbg !11
  %t41 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !11
  %t42 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !11
  call void @llvm.memcpy.p0.p0.i64(ptr %t42, ptr %t41, i64 32, i1 false), !dbg !11
  ret void, !dbg !11
L40:
  %t43 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !11
  %t44 = load i64, ptr %p0, align 8, !dbg !15
  store ptr @ignite.empty, ptr %t45, align 8, !dbg !16
  %t46 = getelementptr inbounds i8, ptr %t45, i64 8, !dbg !16
  store i64 0, ptr %t46, align 8, !dbg !16
  %t47 = getelementptr inbounds i8, ptr %t45, i64 16, !dbg !16
  store i64 0, ptr %t47, align 8, !dbg !16
  call void @"copy.Text"(ptr %t48, ptr %t45), !dbg !16
  %t52 = call i32 @ignite_rt_directory_read_directory(i64 %t44, ptr %t48, ptr %t50, ptr %t51), !dbg !15
  %t53 = icmp ne i32 %t52, 0, !dbg !15
  br i1 %t53, label %L54, label %L55, !dbg !15
L54:
  store i32 1, ptr %t49, align 4, !dbg !15
  %t57 = getelementptr inbounds i8, ptr %t49, i64 8, !dbg !15
  call void @llvm.memcpy.p0.p0.i64(ptr %t57, ptr %t51, i64 32, i1 false), !dbg !15
  br label %L56, !dbg !15
L55:
  store i32 0, ptr %t49, align 4, !dbg !15
  %t58 = getelementptr inbounds i8, ptr %t49, i64 8, !dbg !15
  call void @llvm.memcpy.p0.p0.i64(ptr %t58, ptr %t50, i64 24, i1 false), !dbg !15
  br label %L56, !dbg !15
L56:
  %t59 = load i32, ptr %t49, align 4, !dbg !15
  %t60 = icmp eq i32 %t59, 1, !dbg !15
  br i1 %t60, label %L61, label %L62, !dbg !15
L61:
  store i32 1, ptr %sret, align 4, !dbg !15
  %t63 = getelementptr inbounds i8, ptr %t49, i64 8, !dbg !15
  %t64 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !15
  call void @llvm.memcpy.p0.p0.i64(ptr %t64, ptr %t63, i64 32, i1 false), !dbg !15
  ret void, !dbg !15
L62:
  %t65 = getelementptr inbounds i8, ptr %t49, i64 8, !dbg !15
  call void @llvm.memcpy.p0.p0.i64(ptr %t66, ptr %t65, i64 24, i1 false), !dbg !14
  %t67 = load i64, ptr %p0, align 8, !dbg !17
  store ptr @.str.3, ptr %t68, align 8, !dbg !18
  %t69 = getelementptr inbounds i8, ptr %t68, i64 8, !dbg !18
  store i64 4, ptr %t69, align 8, !dbg !18
  %t70 = getelementptr inbounds i8, ptr %t68, i64 16, !dbg !18
  store i64 4, ptr %t70, align 8, !dbg !18
  call void @"copy.Text"(ptr %t71, ptr %t68), !dbg !18
  %t74 = call i32 @ignite_rt_directory_create_directory(i64 %t67, ptr %t71, ptr %t73), !dbg !17
  %t75 = icmp ne i32 %t74, 0, !dbg !17
  br i1 %t75, label %L76, label %L77, !dbg !17
L76:
  store i32 1, ptr %t72, align 4, !dbg !17
  %t79 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !17
  call void @llvm.memcpy.p0.p0.i64(ptr %t79, ptr %t73, i64 32, i1 false), !dbg !17
  br label %L78, !dbg !17
L77:
  store i32 0, ptr %t72, align 4, !dbg !17
  br label %L78, !dbg !17
L78:
  %t80 = load i32, ptr %t72, align 4, !dbg !17
  %t81 = icmp eq i32 %t80, 1, !dbg !17
  br i1 %t81, label %L82, label %L83, !dbg !17
L82:
  store i32 1, ptr %sret, align 4, !dbg !17
  %t84 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !17
  %t85 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !17
  call void @llvm.memcpy.p0.p0.i64(ptr %t85, ptr %t84, i64 32, i1 false), !dbg !17
  ret void, !dbg !17
L83:
  %t86 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !17
  %t87 = load i64, ptr %p0, align 8, !dbg !19
  store ptr @.str.2, ptr %t88, align 8, !dbg !20
  %t89 = getelementptr inbounds i8, ptr %t88, i64 8, !dbg !20
  store i64 7, ptr %t89, align 8, !dbg !20
  %t90 = getelementptr inbounds i8, ptr %t88, i64 16, !dbg !20
  store i64 7, ptr %t90, align 8, !dbg !20
  call void @"copy.Text"(ptr %t91, ptr %t88), !dbg !20
  %t94 = call i32 @ignite_rt_directory_remove(i64 %t87, ptr %t91, ptr %t93), !dbg !19
  %t95 = icmp ne i32 %t94, 0, !dbg !19
  br i1 %t95, label %L96, label %L97, !dbg !19
L96:
  store i32 1, ptr %t92, align 4, !dbg !19
  %t99 = getelementptr inbounds i8, ptr %t92, i64 8, !dbg !19
  call void @llvm.memcpy.p0.p0.i64(ptr %t99, ptr %t93, i64 32, i1 false), !dbg !19
  br label %L98, !dbg !19
L97:
  store i32 0, ptr %t92, align 4, !dbg !19
  br label %L98, !dbg !19
L98:
  %t100 = load i32, ptr %t92, align 4, !dbg !19
  %t101 = icmp eq i32 %t100, 1, !dbg !19
  br i1 %t101, label %L102, label %L103, !dbg !19
L102:
  store i32 1, ptr %sret, align 4, !dbg !19
  %t104 = getelementptr inbounds i8, ptr %t92, i64 8, !dbg !19
  %t105 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !19
  call void @llvm.memcpy.p0.p0.i64(ptr %t105, ptr %t104, i64 32, i1 false), !dbg !19
  ret void, !dbg !19
L103:
  %t106 = getelementptr inbounds i8, ptr %t92, i64 8, !dbg !19
  %t107 = load i64, ptr %p0, align 8, !dbg !21
  store ptr @.str.3, ptr %t108, align 8, !dbg !22
  %t109 = getelementptr inbounds i8, ptr %t108, i64 8, !dbg !22
  store i64 4, ptr %t109, align 8, !dbg !22
  %t110 = getelementptr inbounds i8, ptr %t108, i64 16, !dbg !22
  store i64 4, ptr %t110, align 8, !dbg !22
  call void @"copy.Text"(ptr %t111, ptr %t108), !dbg !22
  %t114 = call i32 @ignite_rt_directory_remove_tree(i64 %t107, ptr %t111, ptr %t113), !dbg !21
  %t115 = icmp ne i32 %t114, 0, !dbg !21
  br i1 %t115, label %L116, label %L117, !dbg !21
L116:
  store i32 1, ptr %t112, align 4, !dbg !21
  %t119 = getelementptr inbounds i8, ptr %t112, i64 8, !dbg !21
  call void @llvm.memcpy.p0.p0.i64(ptr %t119, ptr %t113, i64 32, i1 false), !dbg !21
  br label %L118, !dbg !21
L117:
  store i32 0, ptr %t112, align 4, !dbg !21
  br label %L118, !dbg !21
L118:
  %t120 = load i32, ptr %t112, align 4, !dbg !21
  %t121 = icmp eq i32 %t120, 1, !dbg !21
  br i1 %t121, label %L122, label %L123, !dbg !21
L122:
  store i32 1, ptr %sret, align 4, !dbg !21
  %t124 = getelementptr inbounds i8, ptr %t112, i64 8, !dbg !21
  %t125 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !21
  call void @llvm.memcpy.p0.p0.i64(ptr %t125, ptr %t124, i64 32, i1 false), !dbg !21
  ret void, !dbg !21
L123:
  %t126 = getelementptr inbounds i8, ptr %t112, i64 8, !dbg !21
  %t127 = load i64, ptr %p0, align 8, !dbg !24
  %t131 = call i32 @ignite_rt_directory_absolute_path(i64 %t127, ptr %t129, ptr %t130), !dbg !24
  %t132 = icmp ne i32 %t131, 0, !dbg !24
  br i1 %t132, label %L133, label %L134, !dbg !24
L133:
  store i32 1, ptr %t128, align 4, !dbg !24
  %t136 = getelementptr inbounds i8, ptr %t128, i64 8, !dbg !24
  call void @llvm.memcpy.p0.p0.i64(ptr %t136, ptr %t130, i64 32, i1 false), !dbg !24
  br label %L135, !dbg !24
L134:
  store i32 0, ptr %t128, align 4, !dbg !24
  %t137 = getelementptr inbounds i8, ptr %t128, i64 8, !dbg !24
  call void @llvm.memcpy.p0.p0.i64(ptr %t137, ptr %t129, i64 24, i1 false), !dbg !24
  br label %L135, !dbg !24
L135:
  %t138 = load i32, ptr %t128, align 4, !dbg !24
  %t139 = icmp eq i32 %t138, 1, !dbg !24
  br i1 %t139, label %L140, label %L141, !dbg !24
L140:
  store i32 1, ptr %sret, align 4, !dbg !24
  %t142 = getelementptr inbounds i8, ptr %t128, i64 8, !dbg !24
  %t143 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !24
  call void @llvm.memcpy.p0.p0.i64(ptr %t143, ptr %t142, i64 32, i1 false), !dbg !24
  ret void, !dbg !24
L141:
  %t144 = getelementptr inbounds i8, ptr %t128, i64 8, !dbg !24
  call void @llvm.memcpy.p0.p0.i64(ptr %t145, ptr %t144, i64 24, i1 false), !dbg !23
  %t147 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !27
  %t148 = load i64, ptr %t147, align 8, !dbg !27
  call void @ignite_rt_setting_value(ptr %t149, i64 %t148), !dbg !27
  call void @llvm.memcpy.p0.p0.i64(ptr %t146, ptr %t149, i64 24, i1 false), !dbg !26
  %t150 = call ptr @ignite.alloc(i64 48, i64 8), !dbg !28
  store ptr @.str.4, ptr %t151, align 8, !dbg !29
  %t152 = getelementptr inbounds i8, ptr %t151, i64 8, !dbg !29
  store i64 2, ptr %t152, align 8, !dbg !29
  %t153 = getelementptr inbounds i8, ptr %t151, i64 16, !dbg !29
  store i64 2, ptr %t153, align 8, !dbg !29
  call void @"copy.Text"(ptr %t154, ptr %t151), !dbg !29
  call void @llvm.memcpy.p0.p0.i64(ptr %t150, ptr %t154, i64 24, i1 false), !dbg !28
  store ptr @.str.5, ptr %t155, align 8, !dbg !30
  %t156 = getelementptr inbounds i8, ptr %t155, i64 8, !dbg !30
  store i64 4, ptr %t156, align 8, !dbg !30
  %t157 = getelementptr inbounds i8, ptr %t155, i64 16, !dbg !30
  store i64 4, ptr %t157, align 8, !dbg !30
  call void @"copy.Text"(ptr %t158, ptr %t155), !dbg !30
  %t159 = getelementptr inbounds i8, ptr %t150, i64 24, !dbg !28
  call void @llvm.memcpy.p0.p0.i64(ptr %t159, ptr %t158, i64 24, i1 false), !dbg !28
  store ptr %t150, ptr %t160, align 8, !dbg !28
  %t161 = getelementptr inbounds i8, ptr %t160, i64 8, !dbg !28
  store i64 2, ptr %t161, align 8, !dbg !28
  %t162 = getelementptr inbounds i8, ptr %t160, i64 16, !dbg !28
  store i64 2, ptr %t162, align 8, !dbg !28
  %t163 = getelementptr inbounds i8, ptr %t146, i64 24, !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t163, ptr %t160, i64 24, i1 false), !dbg !26
  %t164 = getelementptr inbounds i8, ptr %t146, i64 48, !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t164, ptr %t145, i64 24, i1 false), !dbg !26
  call void @"std.process.Invocation.environment"(ptr %t165), !dbg !26
  %t166 = getelementptr inbounds i8, ptr %t146, i64 72, !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t166, ptr %t165, i64 24, i1 false), !dbg !26
  %t167 = call i1 @"std.process.Invocation.inherit_environment"(), !dbg !26
  %t168 = getelementptr inbounds i8, ptr %t146, i64 96, !dbg !26
  %t169 = zext i1 %t167 to i8, !dbg !26
  store i8 %t169, ptr %t168, align 1, !dbg !26
  %t170 = call i64 @"std.process.Invocation.timeout"(), !dbg !26
  %t171 = getelementptr inbounds i8, ptr %t146, i64 104, !dbg !26
  store i64 %t170, ptr %t171, align 8, !dbg !26
  %t172 = call i64 @"std.process.Invocation.output_limit"(), !dbg !26
  %t173 = getelementptr inbounds i8, ptr %t146, i64 112, !dbg !26
  store i64 %t172, ptr %t173, align 8, !dbg !26
  call void @"std.process.Invocation.input"(ptr %t174), !dbg !26
  %t175 = getelementptr inbounds i8, ptr %t146, i64 120, !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t175, ptr %t174, i64 24, i1 false), !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t176, ptr %t146, i64 144, i1 false), !dbg !25
  %t177 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !33
  %t178 = load i64, ptr %t177, align 8, !dbg !33
  %t182 = call i32 @ignite_rt_launcher_execute(i64 %t178, ptr %t176, ptr %t180, ptr %t181), !dbg !33
  %t183 = icmp ne i32 %t182, 0, !dbg !33
  br i1 %t183, label %L184, label %L185, !dbg !33
L184:
  store i32 1, ptr %t179, align 4, !dbg !33
  %t187 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !33
  call void @llvm.memcpy.p0.p0.i64(ptr %t187, ptr %t181, i64 32, i1 false), !dbg !33
  br label %L186, !dbg !33
L185:
  store i32 0, ptr %t179, align 4, !dbg !33
  %t188 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !33
  call void @llvm.memcpy.p0.p0.i64(ptr %t188, ptr %t180, i64 72, i1 false), !dbg !33
  br label %L186, !dbg !33
L186:
  %t189 = load i32, ptr %t179, align 4, !dbg !33
  %t190 = icmp eq i32 %t189, 1, !dbg !33
  br i1 %t190, label %L191, label %L192, !dbg !33
L191:
  store i32 1, ptr %sret, align 4, !dbg !33
  %t193 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !33
  %t194 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !33
  call void @llvm.memcpy.p0.p0.i64(ptr %t194, ptr %t193, i64 32, i1 false), !dbg !33
  ret void, !dbg !33
L192:
  %t195 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !33
  call void @llvm.memcpy.p0.p0.i64(ptr %t196, ptr %t195, i64 72, i1 false), !dbg !32
  %t197 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !36
  %t198 = load i64, ptr %t197, align 8, !dbg !36
  %t202 = call i32 @ignite_rt_launcher_start(i64 %t198, ptr %t176, ptr %t200, ptr %t201), !dbg !36
  %t203 = icmp ne i32 %t202, 0, !dbg !36
  br i1 %t203, label %L204, label %L205, !dbg !36
L204:
  store i32 1, ptr %t199, align 4, !dbg !36
  %t207 = getelementptr inbounds i8, ptr %t199, i64 8, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t207, ptr %t201, i64 32, i1 false), !dbg !36
  br label %L206, !dbg !36
L205:
  store i32 0, ptr %t199, align 4, !dbg !36
  %t208 = getelementptr inbounds i8, ptr %t199, i64 8, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t208, ptr %t200, i64 8, i1 false), !dbg !36
  br label %L206, !dbg !36
L206:
  %t209 = load i32, ptr %t199, align 4, !dbg !36
  %t210 = icmp eq i32 %t209, 1, !dbg !36
  br i1 %t210, label %L211, label %L212, !dbg !36
L211:
  store i32 1, ptr %sret, align 4, !dbg !36
  %t213 = getelementptr inbounds i8, ptr %t199, i64 8, !dbg !36
  %t214 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t214, ptr %t213, i64 32, i1 false), !dbg !36
  ret void, !dbg !36
L212:
  %t215 = getelementptr inbounds i8, ptr %t199, i64 8, !dbg !36
  %t216 = load i64, ptr %t215, align 8, !dbg !36
  store i64 %t216, ptr %t217, align 8, !dbg !35
  %t218 = load i64, ptr %t217, align 8, !dbg !39
  %t219 = call i64 @ignite_rt_child_id(i64 %t218), !dbg !39
  store i64 %t219, ptr %t220, align 8, !dbg !38
  %t221 = load i64, ptr %t217, align 8, !dbg !40
  %t224 = call i32 @ignite_rt_child_kill(i64 %t221, ptr %t223), !dbg !40
  %t225 = icmp ne i32 %t224, 0, !dbg !40
  br i1 %t225, label %L226, label %L227, !dbg !40
L226:
  store i32 1, ptr %t222, align 4, !dbg !40
  %t229 = getelementptr inbounds i8, ptr %t222, i64 8, !dbg !40
  call void @llvm.memcpy.p0.p0.i64(ptr %t229, ptr %t223, i64 32, i1 false), !dbg !40
  br label %L228, !dbg !40
L227:
  store i32 0, ptr %t222, align 4, !dbg !40
  br label %L228, !dbg !40
L228:
  %t230 = load i32, ptr %t222, align 4, !dbg !40
  %t231 = icmp eq i32 %t230, 1, !dbg !40
  br i1 %t231, label %L232, label %L233, !dbg !40
L232:
  store i32 1, ptr %sret, align 4, !dbg !40
  %t234 = getelementptr inbounds i8, ptr %t222, i64 8, !dbg !40
  %t235 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !40
  call void @llvm.memcpy.p0.p0.i64(ptr %t235, ptr %t234, i64 32, i1 false), !dbg !40
  ret void, !dbg !40
L233:
  %t236 = getelementptr inbounds i8, ptr %t222, i64 8, !dbg !40
  %t237 = load i64, ptr %t217, align 8, !dbg !42
  %t241 = call i32 @ignite_rt_child_wait(i64 %t237, ptr %t239, ptr %t240), !dbg !42
  %t242 = icmp ne i32 %t241, 0, !dbg !42
  br i1 %t242, label %L243, label %L244, !dbg !42
L243:
  store i32 1, ptr %t238, align 4, !dbg !42
  %t246 = getelementptr inbounds i8, ptr %t238, i64 8, !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t246, ptr %t240, i64 32, i1 false), !dbg !42
  br label %L245, !dbg !42
L244:
  store i32 0, ptr %t238, align 4, !dbg !42
  %t247 = getelementptr inbounds i8, ptr %t238, i64 8, !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t247, ptr %t239, i64 72, i1 false), !dbg !42
  br label %L245, !dbg !42
L245:
  %t248 = load i32, ptr %t238, align 4, !dbg !42
  %t249 = icmp eq i32 %t248, 1, !dbg !42
  br i1 %t249, label %L250, label %L251, !dbg !42
L250:
  store i32 1, ptr %sret, align 4, !dbg !42
  %t252 = getelementptr inbounds i8, ptr %t238, i64 8, !dbg !42
  %t253 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t253, ptr %t252, i64 32, i1 false), !dbg !42
  ret void, !dbg !42
L251:
  %t254 = getelementptr inbounds i8, ptr %t238, i64 8, !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t255, ptr %t254, i64 72, i1 false), !dbg !41
  %t256 = getelementptr inbounds i8, ptr %p0, i64 16, !dbg !44
  %t257 = load i64, ptr %t256, align 8, !dbg !44
  %t261 = call i32 @ignite_rt_entropy_random_bytes(i64 %t257, i64 32, ptr %t259, ptr %t260), !dbg !44
  %t262 = icmp ne i32 %t261, 0, !dbg !44
  br i1 %t262, label %L263, label %L264, !dbg !44
L263:
  store i32 1, ptr %t258, align 4, !dbg !44
  %t266 = getelementptr inbounds i8, ptr %t258, i64 8, !dbg !44
  call void @llvm.memcpy.p0.p0.i64(ptr %t266, ptr %t260, i64 32, i1 false), !dbg !44
  br label %L265, !dbg !44
L264:
  store i32 0, ptr %t258, align 4, !dbg !44
  %t267 = getelementptr inbounds i8, ptr %t258, i64 8, !dbg !44
  call void @llvm.memcpy.p0.p0.i64(ptr %t267, ptr %t259, i64 24, i1 false), !dbg !44
  br label %L265, !dbg !44
L265:
  %t268 = load i32, ptr %t258, align 4, !dbg !44
  %t269 = icmp eq i32 %t268, 1, !dbg !44
  br i1 %t269, label %L270, label %L271, !dbg !44
L270:
  store i32 1, ptr %sret, align 4, !dbg !44
  %t272 = getelementptr inbounds i8, ptr %t258, i64 8, !dbg !44
  %t273 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !44
  call void @llvm.memcpy.p0.p0.i64(ptr %t273, ptr %t272, i64 32, i1 false), !dbg !44
  ret void, !dbg !44
L271:
  %t274 = getelementptr inbounds i8, ptr %t258, i64 8, !dbg !44
  call void @llvm.memcpy.p0.p0.i64(ptr %t275, ptr %t274, i64 24, i1 false), !dbg !43
  store i32 0, ptr %t276, align 4, !dbg !46
  %t277 = getelementptr inbounds i8, ptr %t276, i64 8, !dbg !46
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t276, i64 40, i1 false), !dbg !7
  ret void, !dbg !7
}

define internal void @"copy.Text"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

!llvm.dbg.cu = !{!1}
!llvm.module.flags = !{!48, !49}
!0 = !DIFile(filename: "golden.capabilities.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "capabilities.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "golden.capabilities.Bench.executable", linkageName: "golden.capabilities.Bench.executable", scope: !3, file: !3, line: 15, type: !2, scopeLine: 15, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 15, column: 22, scope: !4)
!6 = distinct !DISubprogram(name: "golden.capabilities.Bench.main", linkageName: "golden.capabilities.Bench.main", scope: !3, file: !3, line: 18, type: !2, scopeLine: 18, spFlags: DISPFlagDefinition, unit: !1)
!7 = !DILocation(line: 18, column: 53, scope: !6)
!8 = !DILocation(line: 19, column: 3, scope: !6)
!9 = !DILocation(line: 19, column: 14, scope: !6)
!10 = !DILocation(line: 19, column: 35, scope: !6)
!11 = !DILocation(line: 20, column: 3, scope: !6)
!12 = !DILocation(line: 20, column: 25, scope: !6)
!13 = !DILocation(line: 20, column: 36, scope: !6)
!14 = !DILocation(line: 21, column: 3, scope: !6)
!15 = !DILocation(line: 21, column: 15, scope: !6)
!16 = !DILocation(line: 21, column: 41, scope: !6)
!17 = !DILocation(line: 22, column: 3, scope: !6)
!18 = !DILocation(line: 22, column: 31, scope: !6)
!19 = !DILocation(line: 23, column: 3, scope: !6)
!20 = !DILocation(line: 23, column: 21, scope: !6)
!21 = !DILocation(line: 24, column: 3, scope: !6)
!22 = !DILocation(line: 24, column: 26, scope: !6)
!23 = !DILocation(line: 25, column: 3, scope: !6)
!24 = !DILocation(line: 25, column: 14, scope: !6)
!25 = !DILocation(line: 26, column: 3, scope: !6)
!26 = !DILocation(line: 26, column: 20, scope: !6)
!27 = !DILocation(line: 26, column: 43, scope: !6)
!28 = !DILocation(line: 26, column: 74, scope: !6)
!29 = !DILocation(line: 26, column: 75, scope: !6)
!30 = !DILocation(line: 26, column: 81, scope: !6)
!31 = !DILocation(line: 26, column: 102, scope: !6)
!32 = !DILocation(line: 27, column: 3, scope: !6)
!33 = !DILocation(line: 27, column: 13, scope: !6)
!34 = !DILocation(line: 27, column: 32, scope: !6)
!35 = !DILocation(line: 28, column: 3, scope: !6)
!36 = !DILocation(line: 28, column: 15, scope: !6)
!37 = !DILocation(line: 28, column: 32, scope: !6)
!38 = !DILocation(line: 29, column: 3, scope: !6)
!39 = !DILocation(line: 29, column: 12, scope: !6)
!40 = !DILocation(line: 30, column: 3, scope: !6)
!41 = !DILocation(line: 31, column: 3, scope: !6)
!42 = !DILocation(line: 31, column: 16, scope: !6)
!43 = !DILocation(line: 32, column: 3, scope: !6)
!44 = !DILocation(line: 32, column: 15, scope: !6)
!45 = !DILocation(line: 32, column: 41, scope: !6)
!46 = !DILocation(line: 33, column: 3, scope: !6)
!47 = !DILocation(line: 33, column: 6, scope: !6)
!48 = !{i32 7, !"Dwarf Version", i32 5}
!49 = !{i32 2, !"Debug Info Version", i32 3}
