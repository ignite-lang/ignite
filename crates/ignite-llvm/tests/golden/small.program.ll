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
declare void @"ignite_rt_fail_arith"(ptr, ptr, i64, i64, i64, i64)
declare void @"ignite_rt_fail_match"(ptr, i64, i64)
declare i64 @"ignite_rt_own_mark"()
declare void @"ignite_rt_own_release"(i64)
declare void @"ignite_rt_tcp_own"(i64)

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
  %big = icmp sgt i64 %four, 16384
  %limit = select i1 %big, i64 %four, i64 16384
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
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t2, i64 24, i1 false), !dbg !18
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
  %t35 = alloca [24 x i8], align 8
  %t38 = alloca [24 x i8], align 8
  %t47 = alloca [8 x i8], align 8
  %t61 = alloca [1 x i8], align 1
  %t69 = alloca [24 x i8], align 8
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
  %t19 = load ptr, ptr %t6, align 8, !dbg !36
  %t20 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !36
  %t21 = load i64, ptr %t20, align 8, !dbg !36
  %t22 = load ptr, ptr %p0, align 8, !dbg !36
  %t23 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !36
  %t24 = load i64, ptr %t23, align 8, !dbg !36
  %t25 = add i64 %t21, %t24, !dbg !36
  %t26 = call ptr @ignite.alloc(i64 %t25, i64 1), !dbg !36
  store ptr %t26, ptr %t18, align 8, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t26, ptr %t19, i64 %t21, i1 false), !dbg !36
  %t27 = getelementptr inbounds i8, ptr %t26, i64 %t21, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t27, ptr %t22, i64 %t24, i1 false), !dbg !36
  %t28 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !36
  store i64 %t25, ptr %t28, align 8, !dbg !36
  %t29 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !36
  store i64 %t25, ptr %t29, align 8, !dbg !36
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t18, i64 24, i1 false), !dbg !35
  %t30 = load i64, ptr %t7, align 8, !dbg !39
  %t31 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t30, i64 1), !dbg !39
  %t32 = extractvalue { i64, i1 } %t31, 1, !dbg !39
  br i1 %t32, label %L33, label %L34, !dbg !39
L33:
  store ptr @.str.0, ptr %t35, align 8, !dbg !39
  %t36 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !39
  store i64 25, ptr %t36, align 8, !dbg !39
  %t37 = getelementptr inbounds i8, ptr %t35, i64 16, !dbg !39
  store i64 25, ptr %t37, align 8, !dbg !39
  store ptr @.str.3, ptr %t38, align 8, !dbg !39
  %t39 = getelementptr inbounds i8, ptr %t38, i64 8, !dbg !39
  store i64 1, ptr %t39, align 8, !dbg !39
  %t40 = getelementptr inbounds i8, ptr %t38, i64 16, !dbg !39
  store i64 1, ptr %t40, align 8, !dbg !39
  call void @ignite_rt_fail_arith(ptr %t38, ptr %t35, i64 27, i64 9, i64 %t30, i64 1), !dbg !39
  unreachable, !dbg !39
L34:
  %t41 = extractvalue { i64, i1 } %t31, 0, !dbg !39
  store i64 %t41, ptr %t7, align 8, !dbg !38
  br label %L13
L13:
  %t42 = load i64, ptr %t10, align 8, !dbg !32
  %t43 = call i1 @ignite.arena_due(i64 %t9, i64 %t42), !dbg !32
  br i1 %t43, label %L44, label %L11, !dbg !32
L44:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !32
  call void @"copy.Text"(ptr %t8, ptr %t6), !dbg !32
  call void @ignite_rt_arena_scratch(i8 0), !dbg !32
  call void @ignite_rt_arena_release(i64 %t9), !dbg !32
  call void @"copy.Text"(ptr %t6, ptr %t8), !dbg !32
  call void @ignite_rt_arena_scratch_clear(), !dbg !32
  %t45 = call i64 @ignite.arena_mark(), !dbg !32
  %t46 = sub i64 %t45, %t9, !dbg !32
  store i64 %t46, ptr %t10, align 8, !dbg !32
  br label %L11, !dbg !32
L14:
  call void @"copy.List<Byte>"(ptr %t38, ptr %p0), !dbg !41
  store i64 0, ptr %t47, align 8, !dbg !41
  %t48 = call i64 @ignite.arena_mark(), !dbg !41
  br label %L49
L49:
  %t53 = load i64, ptr %t47, align 8, !dbg !41
  %t54 = getelementptr inbounds i8, ptr %t38, i64 8, !dbg !41
  %t55 = load i64, ptr %t54, align 8, !dbg !41
  %t56 = icmp slt i64 %t53, %t55, !dbg !41
  br i1 %t56, label %L50, label %L52, !dbg !41
L50:
  %t57 = load ptr, ptr %t38, align 8, !dbg !41
  %t58 = mul i64 %t53, 1, !dbg !41
  %t59 = getelementptr inbounds i8, ptr %t57, i64 %t58, !dbg !41
  %t60 = load i8, ptr %t59, align 1, !dbg !41
  store i8 %t60, ptr %t61, align 1, !dbg !41
  %t62 = load i64, ptr %t7, align 8, !dbg !44
  %t63 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t62, i64 1), !dbg !44
  %t64 = extractvalue { i64, i1 } %t63, 1, !dbg !44
  br i1 %t64, label %L65, label %L66, !dbg !44
L65:
  store ptr @.str.0, ptr %t35, align 8, !dbg !44
  %t67 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !44
  store i64 25, ptr %t67, align 8, !dbg !44
  %t68 = getelementptr inbounds i8, ptr %t35, i64 16, !dbg !44
  store i64 25, ptr %t68, align 8, !dbg !44
  store ptr @.str.3, ptr %t69, align 8, !dbg !44
  %t70 = getelementptr inbounds i8, ptr %t69, i64 8, !dbg !44
  store i64 1, ptr %t70, align 8, !dbg !44
  %t71 = getelementptr inbounds i8, ptr %t69, i64 16, !dbg !44
  store i64 1, ptr %t71, align 8, !dbg !44
  call void @ignite_rt_fail_arith(ptr %t69, ptr %t35, i64 30, i64 9, i64 %t62, i64 1), !dbg !44
  unreachable, !dbg !44
L66:
  %t72 = extractvalue { i64, i1 } %t63, 0, !dbg !44
  store i64 %t72, ptr %t7, align 8, !dbg !43
  br label %L51
L51:
  %t73 = load i64, ptr %t47, align 8, !dbg !41
  %t74 = add i64 %t73, 1, !dbg !41
  store i64 %t74, ptr %t47, align 8, !dbg !41
  call void @ignite.arena_release(i64 %t48), !dbg !41
  br label %L49, !dbg !41
L52:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t6, i64 24, i1 false), !dbg !46
  ret void, !dbg !46
}

define void @"tests.small.shout"(ptr %sret, ptr %p0) !dbg !48 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t2 = load ptr, ptr %p0, align 8, !dbg !49
  %t3 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !49
  %t4 = load i64, ptr %t3, align 8, !dbg !49
  %t5 = add i64 %t4, 1, !dbg !49
  %t6 = call ptr @ignite.alloc(i64 %t5, i64 1), !dbg !49
  store ptr %t6, ptr %t1, align 8, !dbg !49
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t2, i64 %t4, i1 false), !dbg !49
  %t7 = getelementptr inbounds i8, ptr %t6, i64 %t4, !dbg !49
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr @.str.4, i64 1, i1 false), !dbg !49
  %t8 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !49
  store i64 %t5, ptr %t8, align 8, !dbg !49
  %t9 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !49
  store i64 %t5, ptr %t9, align 8, !dbg !49
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !49
  ret void, !dbg !49
}

define void @"tests.small.greet"(ptr %sret, ptr %p0) !dbg !50 {
entry:
  %t1 = alloca [24 x i8], align 8
  call void @"tests.small.shout"(ptr %t1, ptr %p0), !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !51
  ret void, !dbg !51
}

define void @"tests.small.checked"(ptr %sret, i64 %p0) !dbg !53 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  %t11 = alloca [32 x i8], align 8
  %t12 = alloca [40 x i8], align 8
  %t16 = alloca [16 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !54
  %t2 = load i64, ptr %t1, align 8, !dbg !56
  %t3 = icmp slt i64 %t2, 0, !dbg !56
  br i1 %t3, label %L4, label %L5, !dbg !55
L4:
  store ptr @.str.5, ptr %t7, align 8, !dbg !61
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !61
  store i64 8, ptr %t8, align 8, !dbg !61
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !61
  store i64 8, ptr %t9, align 8, !dbg !61
  call void @"copy.Text"(ptr %t10, ptr %t7), !dbg !61
  call void @ignite_rt_io_error_new(ptr %t11, ptr %t10), !dbg !60
  store i32 1, ptr %t12, align 4, !dbg !59
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t13, ptr %t11, i64 32, i1 false), !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t12, i64 40, i1 false), !dbg !58
  ret void, !dbg !58
L14:
  br label %L6, !dbg !55
L5:
  br label %L6, !dbg !55
L6:
  %t15 = load i64, ptr %t1, align 8, !dbg !66
  store i32 1, ptr %t16, align 4, !dbg !65
  %t17 = getelementptr inbounds i8, ptr %t16, i64 8, !dbg !65
  store i64 %t15, ptr %t17, align 8, !dbg !65
  %t18 = call i64 @"tests.small.area"(ptr %t16), !dbg !64
  store i32 0, ptr %t19, align 4, !dbg !63
  %t20 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !63
  store i64 %t18, ptr %t20, align 8, !dbg !63
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t19, i64 40, i1 false), !dbg !62
  ret void, !dbg !62
}

define void @"tests.small.Echo.connect"(ptr %sret, i64 %p0) !dbg !67 {
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
  store i64 %p0, ptr %t1, align 8, !dbg !68
  call void @"tests.small.checked"(ptr %t2, i64 3), !dbg !70
  %t3 = load i32, ptr %t2, align 4, !dbg !70
  %t4 = icmp eq i32 %t3, 1, !dbg !70
  br i1 %t4, label %L5, label %L6, !dbg !70
L5:
  store i32 1, ptr %sret, align 4, !dbg !70
  %t7 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !70
  %t8 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !70
  call void @llvm.memcpy.p0.p0.i64(ptr %t8, ptr %t7, i64 32, i1 false), !dbg !70
  ret void, !dbg !70
L6:
  %t9 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !70
  %t10 = load i64, ptr %t9, align 8, !dbg !70
  store i64 %t10, ptr %t11, align 8, !dbg !69
  %t13 = load i64, ptr %t11, align 8, !dbg !75
  store i64 %t13, ptr %t12, align 8, !dbg !74
  store ptr @.str.6, ptr %t14, align 8, !dbg !76
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !76
  store i64 4, ptr %t15, align 8, !dbg !76
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !76
  store i64 4, ptr %t16, align 8, !dbg !76
  call void @"copy.Text"(ptr %t17, ptr %t14), !dbg !76
  %t18 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !74
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr %t17, i64 24, i1 false), !dbg !74
  call void @"tests.small.label"(ptr %t19, ptr %t12), !dbg !73
  call void @llvm.memcpy.p0.p0.i64(ptr %t20, ptr %t19, i64 24, i1 false), !dbg !72
  %t21 = load i64, ptr %t1, align 8, !dbg !78
  %t24 = call i32 @ignite_rt_tcp_write_all(i64 %t21, ptr %t20, ptr %t23), !dbg !78
  %t25 = icmp ne i32 %t24, 0, !dbg !78
  br i1 %t25, label %L26, label %L27, !dbg !78
L26:
  store i32 1, ptr %t22, align 4, !dbg !78
  %t29 = getelementptr inbounds i8, ptr %t22, i64 8, !dbg !78
  call void @llvm.memcpy.p0.p0.i64(ptr %t29, ptr %t23, i64 32, i1 false), !dbg !78
  br label %L28, !dbg !78
L27:
  store i32 0, ptr %t22, align 4, !dbg !78
  br label %L28, !dbg !78
L28:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t22, i64 40, i1 false), !dbg !77
  ret void, !dbg !77
}

define void @"tests.small.Dialer.main"(ptr %sret, ptr %p0) !dbg !80 {
entry:
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [8 x i8], align 8
  %t5 = alloca [32 x i8], align 8
  %t21 = alloca [8 x i8], align 8
  %t25 = alloca [40 x i8], align 8
  %t1 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !85
  %t2 = load i64, ptr %t1, align 8, !dbg !85
  %t6 = call i32 @ignite_rt_tcp_connect(ptr %p0, i64 %t2, ptr %t4, ptr %t5), !dbg !83
  %t7 = icmp ne i32 %t6, 0, !dbg !83
  br i1 %t7, label %L8, label %L9, !dbg !83
L8:
  store i32 1, ptr %t3, align 4, !dbg !83
  %t11 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !83
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t5, i64 32, i1 false), !dbg !83
  br label %L10, !dbg !83
L9:
  store i32 0, ptr %t3, align 4, !dbg !83
  %t12 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !83
  call void @llvm.memcpy.p0.p0.i64(ptr %t12, ptr %t4, i64 8, i1 false), !dbg !83
  br label %L10, !dbg !83
L10:
  %t13 = load i32, ptr %t3, align 4, !dbg !83
  %t14 = icmp eq i32 %t13, 1, !dbg !83
  br i1 %t14, label %L15, label %L16, !dbg !83
L15:
  store i32 1, ptr %sret, align 4, !dbg !83
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !83
  %t18 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !83
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr %t17, i64 32, i1 false), !dbg !83
  ret void, !dbg !83
L16:
  %t19 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !83
  %t20 = load i64, ptr %t19, align 8, !dbg !83
  store i64 %t20, ptr %t21, align 8, !dbg !82
  %t22 = call i64 @ignite_rt_own_mark(), !dbg !82
  %t23 = load i64, ptr %t21, align 8, !dbg !82
  call void @ignite_rt_tcp_own(i64 %t23), !dbg !82
  %t24 = load i64, ptr %t21, align 8, !dbg !87
  call void @"tests.small.Echo.connect"(ptr %t25, i64 %t24), !dbg !86
  call void @ignite_rt_own_release(i64 %t22), !dbg !81
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t25, i64 40, i1 false), !dbg !81
  ret void, !dbg !81
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
!llvm.module.flags = !{!88, !89}
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
!50 = distinct !DISubprogram(name: "tests.small.greet", linkageName: "tests.small.greet", scope: !3, file: !3, line: 37, type: !2, scopeLine: 37, spFlags: DISPFlagDefinition, unit: !1)
!51 = !DILocation(line: 37, column: 32, scope: !50)
!52 = !DILocation(line: 37, column: 38, scope: !50)
!53 = distinct !DISubprogram(name: "tests.small.checked", linkageName: "tests.small.checked", scope: !3, file: !3, line: 39, type: !2, scopeLine: 39, spFlags: DISPFlagDefinition, unit: !1)
!54 = !DILocation(line: 39, column: 44, scope: !53)
!55 = !DILocation(line: 40, column: 3, scope: !53)
!56 = !DILocation(line: 40, column: 6, scope: !53)
!57 = !DILocation(line: 40, column: 10, scope: !53)
!58 = !DILocation(line: 41, column: 5, scope: !53)
!59 = !DILocation(line: 41, column: 12, scope: !53)
!60 = !DILocation(line: 41, column: 16, scope: !53)
!61 = !DILocation(line: 41, column: 28, scope: !53)
!62 = !DILocation(line: 43, column: 3, scope: !53)
!63 = !DILocation(line: 43, column: 10, scope: !53)
!64 = !DILocation(line: 43, column: 13, scope: !53)
!65 = !DILocation(line: 43, column: 18, scope: !53)
!66 = !DILocation(line: 43, column: 31, scope: !53)
!67 = distinct !DISubprogram(name: "tests.small.Echo.connect", linkageName: "tests.small.Echo.connect", scope: !3, file: !3, line: 47, type: !2, scopeLine: 47, spFlags: DISPFlagDefinition, unit: !1)
!68 = !DILocation(line: 47, column: 82, scope: !67)
!69 = !DILocation(line: 48, column: 5, scope: !67)
!70 = !DILocation(line: 48, column: 16, scope: !67)
!71 = !DILocation(line: 48, column: 24, scope: !67)
!72 = !DILocation(line: 49, column: 5, scope: !67)
!73 = !DILocation(line: 49, column: 16, scope: !67)
!74 = !DILocation(line: 49, column: 22, scope: !67)
!75 = !DILocation(line: 49, column: 36, scope: !67)
!76 = !DILocation(line: 49, column: 50, scope: !67)
!77 = !DILocation(line: 50, column: 5, scope: !67)
!78 = !DILocation(line: 50, column: 12, scope: !67)
!79 = !DILocation(line: 50, column: 29, scope: !67)
!80 = distinct !DISubprogram(name: "tests.small.Dialer.main", linkageName: "tests.small.Dialer.main", scope: !3, file: !3, line: 60, type: !2, scopeLine: 60, spFlags: DISPFlagDefinition, unit: !1)
!81 = !DILocation(line: 60, column: 55, scope: !80)
!82 = !DILocation(line: 61, column: 3, scope: !80)
!83 = !DILocation(line: 61, column: 16, scope: !80)
!84 = !DILocation(line: 61, column: 34, scope: !80)
!85 = !DILocation(line: 61, column: 48, scope: !80)
!86 = !DILocation(line: 62, column: 3, scope: !80)
!87 = !DILocation(line: 62, column: 16, scope: !80)
!88 = !{i32 7, !"Dwarf Version", i32 5}
!89 = !{i32 2, !"Debug Info Version", i32 3}
