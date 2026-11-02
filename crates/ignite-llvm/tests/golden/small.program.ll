target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'tests.small'
source_filename = "tests.small"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.4 = private unnamed_addr constant [1 x i8] c"!", align 1
@.str.1 = private unnamed_addr constant [1 x i8] c"*", align 1
@.str.3 = private unnamed_addr constant [1 x i8] c"+", align 1
@.str.2 = private unnamed_addr constant [1 x i8] c"=", align 1
@.str.6 = private unnamed_addr constant [4 x i8] c"area", align 1
@.str.5 = private unnamed_addr constant [8 x i8] c"negative", align 1
@.str.0 = private unnamed_addr constant [25 x i8] c"tests/golden/small.ignite", align 1

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
declare void @ignite_rt_fail(ptr) noreturn
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
declare void @"ignite_rt_fail_arith"(ptr, ptr, i64, i64, i64, i64)
declare void @"ignite_rt_fail_match"(ptr, i64, i64)

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
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
  br label %done
done:
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
  %fits = icmp ult i64 %end, %li
  %ok = and i1 %small, %fits
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

define i64 @"tests.small.SCALE"() !dbg !4 {
entry:
  ret i64 2, !dbg !5
}

define i64 @"tests.small.area"(ptr %p0) !dbg !6 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t13 = alloca [8 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t4 = load i32, ptr %p0, align 4, !dbg !9
  %t5 = icmp eq i32 %t4, 0, !dbg !9
  br i1 %t5, label %L6, label %L3, !dbg !9
L6:
  store i64 0, ptr %t1, align 8, !dbg !9
  br label %L2, !dbg !9
L3:
  %t8 = load i32, ptr %p0, align 4, !dbg !9
  %t9 = icmp eq i32 %t8, 1, !dbg !9
  br i1 %t9, label %L10, label %L7, !dbg !9
L10:
  %t11 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !9
  %t12 = load i64, ptr %t11, align 8, !dbg !9
  store i64 %t12, ptr %t13, align 8, !dbg !9
  %t14 = call i64 @"tests.small.SCALE"(), !dbg !12
  %t15 = load i64, ptr %t13, align 8, !dbg !13
  %t16 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %t14, i64 %t15), !dbg !12
  %t17 = extractvalue { i64, i1 } %t16, 1, !dbg !12
  br i1 %t17, label %L18, label %L19, !dbg !12
L18:
  store ptr @.str.0, ptr %t20, align 8, !dbg !12
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !12
  store i64 25, ptr %t21, align 8, !dbg !12
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !12
  store i64 25, ptr %t22, align 8, !dbg !12
  store ptr @.str.1, ptr %t23, align 8, !dbg !12
  %t24 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !12
  store i64 1, ptr %t24, align 8, !dbg !12
  %t25 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !12
  store i64 1, ptr %t25, align 8, !dbg !12
  call void @ignite_rt_fail_arith(ptr %t23, ptr %t20, i64 12, i64 24, i64 %t14, i64 %t15), !dbg !12
  unreachable, !dbg !12
L19:
  %t26 = extractvalue { i64, i1 } %t16, 0, !dbg !12
  %t27 = load i64, ptr %t13, align 8, !dbg !14
  %t28 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %t26, i64 %t27), !dbg !12
  %t29 = extractvalue { i64, i1 } %t28, 1, !dbg !12
  br i1 %t29, label %L30, label %L31, !dbg !12
L30:
  store ptr @.str.0, ptr %t23, align 8, !dbg !12
  %t32 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !12
  store i64 25, ptr %t32, align 8, !dbg !12
  %t33 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !12
  store i64 25, ptr %t33, align 8, !dbg !12
  store ptr @.str.1, ptr %t20, align 8, !dbg !12
  %t34 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !12
  store i64 1, ptr %t34, align 8, !dbg !12
  %t35 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !12
  store i64 1, ptr %t35, align 8, !dbg !12
  call void @ignite_rt_fail_arith(ptr %t20, ptr %t23, i64 12, i64 24, i64 %t26, i64 %t27), !dbg !12
  unreachable, !dbg !12
L31:
  %t36 = extractvalue { i64, i1 } %t28, 0, !dbg !12
  store i64 %t36, ptr %t1, align 8, !dbg !9
  br label %L2, !dbg !9
L7:
  store ptr @.str.0, ptr %t20, align 8, !dbg !9
  %t37 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !9
  store i64 25, ptr %t37, align 8, !dbg !9
  %t38 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !9
  store i64 25, ptr %t38, align 8, !dbg !9
  call void @ignite_rt_fail_match(ptr %t20, i64 10, i64 10), !dbg !9
  unreachable, !dbg !9
L2:
  %t39 = load i64, ptr %t1, align 8, !dbg !9
  ret i64 %t39, !dbg !8
}

define void @"tests.small.label"(ptr %sret, ptr %p0) !dbg !15 {
entry:
  %t3 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t16 = alloca [24 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t1 = call ptr @ignite.alloc(i64 24, i64 8), !dbg !18
  %t2 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !19
  call void @"copy.Text"(ptr %t1, ptr %t2), !dbg !18
  store ptr %t1, ptr %t3, align 8, !dbg !18
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !18
  store i64 1, ptr %t4, align 8, !dbg !18
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16, !dbg !18
  store i64 1, ptr %t5, align 8, !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t3, i64 24, i1 false), !dbg !17
  %t7 = load i64, ptr %p0, align 8, !dbg !22
  call void @ignite_rt_int_to_text(ptr %t8, i64 %t7), !dbg !21
  call void @ignite.reserve(ptr %t6, i64 1, i64 24, i64 8), !dbg !20
  %t9 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !20
  %t10 = load i64, ptr %t9, align 8, !dbg !20
  %t11 = load ptr, ptr %t6, align 8, !dbg !20
  %t12 = mul i64 %t10, 24, !dbg !20
  %t13 = getelementptr inbounds i8, ptr %t11, i64 %t12, !dbg !20
  call void @llvm.memcpy.p0.p0.i64(ptr %t13, ptr %t8, i64 24, i1 false), !dbg !20
  %t14 = add i64 %t10, 1, !dbg !20
  %t15 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !20
  store i64 %t14, ptr %t15, align 8, !dbg !20
  store ptr @.str.2, ptr %t16, align 8, !dbg !25
  %t17 = getelementptr inbounds i8, ptr %t16, i64 8, !dbg !25
  store i64 1, ptr %t17, align 8, !dbg !25
  %t18 = getelementptr inbounds i8, ptr %t16, i64 16, !dbg !25
  store i64 1, ptr %t18, align 8, !dbg !25
  call void @"copy.Text"(ptr %t19, ptr %t16), !dbg !25
  call void @ignite_rt_text_join(ptr %t20, ptr %t6, ptr %t19), !dbg !24
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t20, i64 24, i1 false), !dbg !23
  ret void, !dbg !23
}

define void @"tests.small.repeat"(ptr %sret, ptr %p0, i64 %p1) !dbg !26 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t7 = alloca [8 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t10 = alloca [8 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t24 = alloca [24 x i8], align 8
  %t27 = alloca [24 x i8], align 8
  %t36 = alloca [8 x i8], align 8
  %t50 = alloca [1 x i8], align 1
  %t58 = alloca [24 x i8], align 8
  store i64 %p1, ptr %t1, align 8, !dbg !27
  store ptr @ignite.empty, ptr %t2, align 8, !dbg !29
  %t3 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !29
  store i64 0, ptr %t3, align 8, !dbg !29
  %t4 = getelementptr inbounds i8, ptr %t2, i64 16, !dbg !29
  store i64 0, ptr %t4, align 8, !dbg !29
  call void @"copy.Text"(ptr %t5, ptr %t2), !dbg !29
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t5, i64 24, i1 false), !dbg !28
  store i64 0, ptr %t7, align 8, !dbg !30
  %t9 = call i64 @ignite.arena_mark(), !dbg !32
  store i64 0, ptr %t10, align 8, !dbg !32
  br label %L11
L11:
  %t15 = load i64, ptr %t7, align 8, !dbg !33
  %t16 = load i64, ptr %t1, align 8, !dbg !34
  %t17 = icmp slt i64 %t15, %t16, !dbg !33
  br i1 %t17, label %L12, label %L14, !dbg !32
L12:
  call void @ignite_rt_text_concat(ptr %t18, ptr %t6, ptr %p0), !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t18, i64 24, i1 false), !dbg !35
  %t19 = load i64, ptr %t7, align 8, !dbg !39
  %t20 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t19, i64 1), !dbg !39
  %t21 = extractvalue { i64, i1 } %t20, 1, !dbg !39
  br i1 %t21, label %L22, label %L23, !dbg !39
L22:
  store ptr @.str.0, ptr %t24, align 8, !dbg !39
  %t25 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !39
  store i64 25, ptr %t25, align 8, !dbg !39
  %t26 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !39
  store i64 25, ptr %t26, align 8, !dbg !39
  store ptr @.str.3, ptr %t27, align 8, !dbg !39
  %t28 = getelementptr inbounds i8, ptr %t27, i64 8, !dbg !39
  store i64 1, ptr %t28, align 8, !dbg !39
  %t29 = getelementptr inbounds i8, ptr %t27, i64 16, !dbg !39
  store i64 1, ptr %t29, align 8, !dbg !39
  call void @ignite_rt_fail_arith(ptr %t27, ptr %t24, i64 27, i64 9, i64 %t19, i64 1), !dbg !39
  unreachable, !dbg !39
L23:
  %t30 = extractvalue { i64, i1 } %t20, 0, !dbg !39
  store i64 %t30, ptr %t7, align 8, !dbg !38
  br label %L13
L13:
  %t31 = load i64, ptr %t10, align 8, !dbg !32
  %t32 = call i1 @ignite.arena_due(i64 %t9, i64 %t31), !dbg !32
  br i1 %t32, label %L33, label %L11, !dbg !32
L33:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !32
  call void @"copy.Text"(ptr %t8, ptr %t6), !dbg !32
  call void @ignite_rt_arena_scratch(i8 0), !dbg !32
  call void @ignite_rt_arena_release(i64 %t9), !dbg !32
  call void @"copy.Text"(ptr %t6, ptr %t8), !dbg !32
  call void @ignite_rt_arena_scratch_clear(), !dbg !32
  %t34 = call i64 @ignite.arena_mark(), !dbg !32
  %t35 = sub i64 %t34, %t9, !dbg !32
  store i64 %t35, ptr %t10, align 8, !dbg !32
  br label %L11, !dbg !32
L14:
  call void @"copy.List<Byte>"(ptr %t27, ptr %p0), !dbg !41
  store i64 0, ptr %t36, align 8, !dbg !41
  %t37 = call i64 @ignite.arena_mark(), !dbg !41
  br label %L38
L38:
  %t42 = load i64, ptr %t36, align 8, !dbg !41
  %t43 = getelementptr inbounds i8, ptr %t27, i64 8, !dbg !41
  %t44 = load i64, ptr %t43, align 8, !dbg !41
  %t45 = icmp slt i64 %t42, %t44, !dbg !41
  br i1 %t45, label %L39, label %L41, !dbg !41
L39:
  %t46 = load ptr, ptr %t27, align 8, !dbg !41
  %t47 = mul i64 %t42, 1, !dbg !41
  %t48 = getelementptr inbounds i8, ptr %t46, i64 %t47, !dbg !41
  %t49 = load i8, ptr %t48, align 1, !dbg !41
  store i8 %t49, ptr %t50, align 1, !dbg !41
  %t51 = load i64, ptr %t7, align 8, !dbg !44
  %t52 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t51, i64 1), !dbg !44
  %t53 = extractvalue { i64, i1 } %t52, 1, !dbg !44
  br i1 %t53, label %L54, label %L55, !dbg !44
L54:
  store ptr @.str.0, ptr %t24, align 8, !dbg !44
  %t56 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !44
  store i64 25, ptr %t56, align 8, !dbg !44
  %t57 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !44
  store i64 25, ptr %t57, align 8, !dbg !44
  store ptr @.str.3, ptr %t58, align 8, !dbg !44
  %t59 = getelementptr inbounds i8, ptr %t58, i64 8, !dbg !44
  store i64 1, ptr %t59, align 8, !dbg !44
  %t60 = getelementptr inbounds i8, ptr %t58, i64 16, !dbg !44
  store i64 1, ptr %t60, align 8, !dbg !44
  call void @ignite_rt_fail_arith(ptr %t58, ptr %t24, i64 30, i64 9, i64 %t51, i64 1), !dbg !44
  unreachable, !dbg !44
L55:
  %t61 = extractvalue { i64, i1 } %t52, 0, !dbg !44
  store i64 %t61, ptr %t7, align 8, !dbg !43
  br label %L40
L40:
  %t62 = load i64, ptr %t36, align 8, !dbg !41
  %t63 = add i64 %t62, 1, !dbg !41
  store i64 %t63, ptr %t36, align 8, !dbg !41
  call void @ignite.arena_release(i64 %t37), !dbg !41
  br label %L38, !dbg !41
L41:
  call void @"copy.Text"(ptr %sret, ptr %t6), !dbg !46
  ret void, !dbg !46
}

define void @"tests.small.shout"(ptr %sret, ptr %p0) !dbg !48 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t4 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  store ptr @.str.4, ptr %t1, align 8, !dbg !50
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !50
  store i64 1, ptr %t2, align 8, !dbg !50
  %t3 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !50
  store i64 1, ptr %t3, align 8, !dbg !50
  call void @"copy.Text"(ptr %t4, ptr %t1), !dbg !50
  call void @ignite_rt_text_concat(ptr %t5, ptr %p0, ptr %t4), !dbg !49
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t5, i64 24, i1 false), !dbg !49
  ret void, !dbg !49
}

define void @"tests.small.greet"(ptr %sret, ptr %p0) !dbg !51 {
entry:
  %t1 = alloca [24 x i8], align 8
  call void @"tests.small.shout"(ptr %t1, ptr %p0), !dbg !52
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !52
  ret void, !dbg !52
}

define void @"tests.small.checked"(ptr %sret, i64 %p0) !dbg !54 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  %t11 = alloca [32 x i8], align 8
  %t12 = alloca [40 x i8], align 8
  %t16 = alloca [16 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !55
  %t2 = load i64, ptr %t1, align 8, !dbg !57
  %t3 = icmp slt i64 %t2, 0, !dbg !57
  br i1 %t3, label %L4, label %L5, !dbg !56
L4:
  store ptr @.str.5, ptr %t7, align 8, !dbg !62
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !62
  store i64 8, ptr %t8, align 8, !dbg !62
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !62
  store i64 8, ptr %t9, align 8, !dbg !62
  call void @"copy.Text"(ptr %t10, ptr %t7), !dbg !62
  call void @ignite_rt_io_error_new(ptr %t11, ptr %t10), !dbg !61
  store i32 1, ptr %t12, align 4, !dbg !60
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !60
  call void @llvm.memcpy.p0.p0.i64(ptr %t13, ptr %t11, i64 32, i1 false), !dbg !60
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t12, i64 40, i1 false), !dbg !59
  ret void, !dbg !59
L14:
  br label %L6, !dbg !56
L5:
  br label %L6, !dbg !56
L6:
  %t15 = load i64, ptr %t1, align 8, !dbg !67
  store i32 1, ptr %t16, align 4, !dbg !66
  %t17 = getelementptr inbounds i8, ptr %t16, i64 8, !dbg !66
  store i64 %t15, ptr %t17, align 8, !dbg !66
  %t18 = call i64 @"tests.small.area"(ptr %t16), !dbg !65
  store i32 0, ptr %t19, align 4, !dbg !64
  %t20 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !64
  store i64 %t18, ptr %t20, align 8, !dbg !64
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t19, i64 40, i1 false), !dbg !63
  ret void, !dbg !63
}

define void @"tests.small.Echo.connect"(ptr %sret, i64 %p0) !dbg !68 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [40 x i8], align 8
  %t11 = alloca [8 x i8], align 8
  %t12 = alloca [32 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t22 = alloca [40 x i8], align 8
  %t23 = alloca [32 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !69
  call void @"tests.small.checked"(ptr %t2, i64 3), !dbg !71
  %t3 = load i32, ptr %t2, align 4, !dbg !71
  %t4 = icmp eq i32 %t3, 1, !dbg !71
  br i1 %t4, label %L5, label %L6, !dbg !71
L5:
  store i32 1, ptr %sret, align 4, !dbg !71
  %t7 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !71
  %t8 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !71
  call void @llvm.memcpy.p0.p0.i64(ptr %t8, ptr %t7, i64 32, i1 false), !dbg !71
  ret void, !dbg !71
L6:
  %t9 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !71
  %t10 = load i64, ptr %t9, align 8, !dbg !71
  store i64 %t10, ptr %t11, align 8, !dbg !70
  %t13 = load i64, ptr %t11, align 8, !dbg !76
  store i64 %t13, ptr %t12, align 8, !dbg !75
  store ptr @.str.6, ptr %t14, align 8, !dbg !77
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !77
  store i64 4, ptr %t15, align 8, !dbg !77
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !77
  store i64 4, ptr %t16, align 8, !dbg !77
  call void @"copy.Text"(ptr %t17, ptr %t14), !dbg !77
  %t18 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !75
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr %t17, i64 24, i1 false), !dbg !75
  call void @"tests.small.label"(ptr %t19, ptr %t12), !dbg !74
  call void @llvm.memcpy.p0.p0.i64(ptr %t20, ptr %t19, i64 24, i1 false), !dbg !73
  %t21 = load i64, ptr %t1, align 8, !dbg !79
  %t24 = call i32 @ignite_rt_tcp_write_all(i64 %t21, ptr %t20, ptr %t23), !dbg !79
  %t25 = icmp ne i32 %t24, 0, !dbg !79
  br i1 %t25, label %L26, label %L27, !dbg !79
L26:
  store i32 1, ptr %t22, align 4, !dbg !79
  %t29 = getelementptr inbounds i8, ptr %t22, i64 8, !dbg !79
  call void @llvm.memcpy.p0.p0.i64(ptr %t29, ptr %t23, i64 32, i1 false), !dbg !79
  br label %L28, !dbg !79
L27:
  store i32 0, ptr %t22, align 4, !dbg !79
  br label %L28, !dbg !79
L28:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t22, i64 40, i1 false), !dbg !78
  ret void, !dbg !78
}

define void @"tests.small.Dialer.main"(ptr %sret, ptr %p0) !dbg !81 {
entry:
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [8 x i8], align 8
  %t5 = alloca [32 x i8], align 8
  %t21 = alloca [8 x i8], align 8
  %t23 = alloca [40 x i8], align 8
  %t1 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !86
  %t2 = load i64, ptr %t1, align 8, !dbg !86
  %t6 = call i32 @ignite_rt_tcp_connect(ptr %p0, i64 %t2, ptr %t4, ptr %t5), !dbg !84
  %t7 = icmp ne i32 %t6, 0, !dbg !84
  br i1 %t7, label %L8, label %L9, !dbg !84
L8:
  store i32 1, ptr %t3, align 4, !dbg !84
  %t11 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !84
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t5, i64 32, i1 false), !dbg !84
  br label %L10, !dbg !84
L9:
  store i32 0, ptr %t3, align 4, !dbg !84
  %t12 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !84
  call void @llvm.memcpy.p0.p0.i64(ptr %t12, ptr %t4, i64 8, i1 false), !dbg !84
  br label %L10, !dbg !84
L10:
  %t13 = load i32, ptr %t3, align 4, !dbg !84
  %t14 = icmp eq i32 %t13, 1, !dbg !84
  br i1 %t14, label %L15, label %L16, !dbg !84
L15:
  store i32 1, ptr %sret, align 4, !dbg !84
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !84
  %t18 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !84
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr %t17, i64 32, i1 false), !dbg !84
  ret void, !dbg !84
L16:
  %t19 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !84
  %t20 = load i64, ptr %t19, align 8, !dbg !84
  store i64 %t20, ptr %t21, align 8, !dbg !83
  %t22 = load i64, ptr %t21, align 8, !dbg !88
  call void @"tests.small.Echo.connect"(ptr %t23, i64 %t22), !dbg !87
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t23, i64 40, i1 false), !dbg !82
  ret void, !dbg !82
}

define internal void @"copy.List<Byte>"(ptr %dst, ptr %src) {
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
!llvm.module.flags = !{!89, !90}
!0 = !DIFile(filename: "tests.small.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "small.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "tests.small.SCALE", linkageName: "tests.small.SCALE", scope: !3, file: !3, line: 7, type: !2, scopeLine: 7, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 7, column: 20, scope: !4)
!6 = distinct !DISubprogram(name: "tests.small.area", linkageName: "tests.small.area", scope: !3, file: !3, line: 9, type: !2, scopeLine: 9, spFlags: DISPFlagDefinition, unit: !1)
!7 = !DILocation(line: 9, column: 30, scope: !6)
!8 = !DILocation(line: 10, column: 3, scope: !6)
!9 = !DILocation(line: 10, column: 10, scope: !6)
!10 = !DILocation(line: 10, column: 16, scope: !6)
!11 = !DILocation(line: 11, column: 12, scope: !6)
!12 = !DILocation(line: 12, column: 24, scope: !6)
!13 = !DILocation(line: 12, column: 32, scope: !6)
!14 = !DILocation(line: 12, column: 36, scope: !6)
!15 = distinct !DISubprogram(name: "tests.small.label", linkageName: "tests.small.label", scope: !3, file: !3, line: 16, type: !2, scopeLine: 16, spFlags: DISPFlagDefinition, unit: !1)
!16 = !DILocation(line: 16, column: 30, scope: !15)
!17 = !DILocation(line: 17, column: 3, scope: !15)
!18 = !DILocation(line: 17, column: 15, scope: !15)
!19 = !DILocation(line: 17, column: 16, scope: !15)
!20 = !DILocation(line: 18, column: 3, scope: !15)
!21 = !DILocation(line: 18, column: 14, scope: !15)
!22 = !DILocation(line: 18, column: 26, scope: !15)
!23 = !DILocation(line: 19, column: 3, scope: !15)
!24 = !DILocation(line: 19, column: 10, scope: !15)
!25 = !DILocation(line: 19, column: 21, scope: !15)
!26 = distinct !DISubprogram(name: "tests.small.repeat", linkageName: "tests.small.repeat", scope: !3, file: !3, line: 22, type: !2, scopeLine: 22, spFlags: DISPFlagDefinition, unit: !1)
!27 = !DILocation(line: 22, column: 43, scope: !26)
!28 = !DILocation(line: 23, column: 3, scope: !26)
!29 = !DILocation(line: 23, column: 13, scope: !26)
!30 = !DILocation(line: 24, column: 3, scope: !26)
!31 = !DILocation(line: 24, column: 11, scope: !26)
!32 = !DILocation(line: 25, column: 3, scope: !26)
!33 = !DILocation(line: 25, column: 9, scope: !26)
!34 = !DILocation(line: 25, column: 13, scope: !26)
!35 = !DILocation(line: 26, column: 5, scope: !26)
!36 = !DILocation(line: 26, column: 11, scope: !26)
!37 = !DILocation(line: 26, column: 22, scope: !26)
!38 = !DILocation(line: 27, column: 5, scope: !26)
!39 = !DILocation(line: 27, column: 9, scope: !26)
!40 = !DILocation(line: 27, column: 13, scope: !26)
!41 = !DILocation(line: 29, column: 3, scope: !26)
!42 = !DILocation(line: 29, column: 12, scope: !26)
!43 = !DILocation(line: 30, column: 5, scope: !26)
!44 = !DILocation(line: 30, column: 9, scope: !26)
!45 = !DILocation(line: 30, column: 13, scope: !26)
!46 = !DILocation(line: 32, column: 3, scope: !26)
!47 = !DILocation(line: 32, column: 10, scope: !26)
!48 = distinct !DISubprogram(name: "tests.small.shout", linkageName: "tests.small.shout", scope: !3, file: !3, line: 35, type: !2, scopeLine: 35, spFlags: DISPFlagDefinition, unit: !1)
!49 = !DILocation(line: 35, column: 32, scope: !48)
!50 = !DILocation(line: 35, column: 44, scope: !48)
!51 = distinct !DISubprogram(name: "tests.small.greet", linkageName: "tests.small.greet", scope: !3, file: !3, line: 37, type: !2, scopeLine: 37, spFlags: DISPFlagDefinition, unit: !1)
!52 = !DILocation(line: 37, column: 32, scope: !51)
!53 = !DILocation(line: 37, column: 38, scope: !51)
!54 = distinct !DISubprogram(name: "tests.small.checked", linkageName: "tests.small.checked", scope: !3, file: !3, line: 39, type: !2, scopeLine: 39, spFlags: DISPFlagDefinition, unit: !1)
!55 = !DILocation(line: 39, column: 44, scope: !54)
!56 = !DILocation(line: 40, column: 3, scope: !54)
!57 = !DILocation(line: 40, column: 6, scope: !54)
!58 = !DILocation(line: 40, column: 10, scope: !54)
!59 = !DILocation(line: 41, column: 5, scope: !54)
!60 = !DILocation(line: 41, column: 12, scope: !54)
!61 = !DILocation(line: 41, column: 16, scope: !54)
!62 = !DILocation(line: 41, column: 28, scope: !54)
!63 = !DILocation(line: 43, column: 3, scope: !54)
!64 = !DILocation(line: 43, column: 10, scope: !54)
!65 = !DILocation(line: 43, column: 13, scope: !54)
!66 = !DILocation(line: 43, column: 18, scope: !54)
!67 = !DILocation(line: 43, column: 31, scope: !54)
!68 = distinct !DISubprogram(name: "tests.small.Echo.connect", linkageName: "tests.small.Echo.connect", scope: !3, file: !3, line: 47, type: !2, scopeLine: 47, spFlags: DISPFlagDefinition, unit: !1)
!69 = !DILocation(line: 47, column: 82, scope: !68)
!70 = !DILocation(line: 48, column: 5, scope: !68)
!71 = !DILocation(line: 48, column: 16, scope: !68)
!72 = !DILocation(line: 48, column: 24, scope: !68)
!73 = !DILocation(line: 49, column: 5, scope: !68)
!74 = !DILocation(line: 49, column: 16, scope: !68)
!75 = !DILocation(line: 49, column: 22, scope: !68)
!76 = !DILocation(line: 49, column: 36, scope: !68)
!77 = !DILocation(line: 49, column: 50, scope: !68)
!78 = !DILocation(line: 50, column: 5, scope: !68)
!79 = !DILocation(line: 50, column: 12, scope: !68)
!80 = !DILocation(line: 50, column: 29, scope: !68)
!81 = distinct !DISubprogram(name: "tests.small.Dialer.main", linkageName: "tests.small.Dialer.main", scope: !3, file: !3, line: 60, type: !2, scopeLine: 60, spFlags: DISPFlagDefinition, unit: !1)
!82 = !DILocation(line: 60, column: 55, scope: !81)
!83 = !DILocation(line: 61, column: 3, scope: !81)
!84 = !DILocation(line: 61, column: 16, scope: !81)
!85 = !DILocation(line: 61, column: 34, scope: !81)
!86 = !DILocation(line: 61, column: 48, scope: !81)
!87 = !DILocation(line: 62, column: 3, scope: !81)
!88 = !DILocation(line: 62, column: 16, scope: !81)
!89 = !{i32 7, !"Dwarf Version", i32 5}
!90 = !{i32 2, !"Debug Info Version", i32 3}
